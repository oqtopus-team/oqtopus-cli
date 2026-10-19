//! HTTPS transport and Git smart-HTTP ref discovery shared by commands that read remote
//! repositories.
//!
//! Requests trust the operating system's certificate store and honor the proxy variables curl
//! reads, so networks with a private CA or an egress proxy work as they did with curl.

use std::env;
use std::io::Read;

use percent_encoding::percent_decode_str;
use ureq::http::Uri;
use ureq::tls::{RootCerts, TlsConfig};
use ureq::{Agent, Proxy, ProxyProtocol};

const GITHUB_BASE_URL: &str = "https://github.com";

pub(crate) fn remote_refs_url(repository: &str) -> String {
    format!("{GITHUB_BASE_URL}/{repository}.git/info/refs?service=git-upload-pack")
}

pub(crate) fn fetch_remote_tags(repository: &str) -> Result<Vec<String>, ()> {
    let advertisement = fetch_url(&remote_refs_url(repository))?;
    let tags = parse_remote_refs(&advertisement)
        .into_iter()
        .filter_map(|(_, reference)| reference.strip_prefix("refs/tags/").map(str::to_owned))
        // Git advertises an annotated tag twice: once as the tag object and once as `<tag>^{}`,
        // the commit it points at. Keep only the tag name itself.
        .filter(|tag| !tag.ends_with("^{}"))
        .collect::<Vec<_>>();
    let mut tags = tags;
    tags.sort();
    tags.dedup();
    (!tags.is_empty()).then_some(tags).ok_or(())
}

pub(crate) fn resolve_branch_commit(repository: &str, branch: &str) -> Result<String, ()> {
    let advertisement = fetch_url(&remote_refs_url(repository))?;
    let expected = format!("refs/heads/{branch}");
    parse_remote_refs(&advertisement)
        .into_iter()
        .find_map(|(sha, reference)| (reference == expected).then_some(sha))
        .ok_or(())
}

pub(crate) fn fetch_url(url: &str) -> Result<Vec<u8>, ()> {
    #[cfg(feature = "test-hooks")]
    if let Some(fixture) = crate::test_hooks::http_fixture(url) {
        return fixture;
    }
    let mut response = agent()?.get(url).call().map_err(|_| ())?;
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut body)
        .map_err(|_| ())?;
    Ok(body)
}

fn agent() -> Result<Agent, ()> {
    let proxy = https_proxy(|name| env::var(name).ok())?;
    Ok(Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .proxy(proxy)
        .build()
        .new_agent())
}

/// Selects the proxy for HTTPS requests from the variables curl reads, in curl's order.
///
/// Every URL this CLI fetches, including redirect targets, is HTTPS, so `http_proxy` never
/// applies. Empty variables count as unset, and an unparsable proxy fails the request rather
/// than silently connecting directly.
fn https_proxy(var: impl Fn(&str) -> Option<String>) -> Result<Option<Proxy>, ()> {
    let set = |name: &str| var(name).filter(|value| !value.is_empty());
    let Some(spec) = ["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"]
        .into_iter()
        .find_map(set)
    else {
        return Ok(None);
    };
    let no_proxy = set("no_proxy")
        .or_else(|| set("NO_PROXY"))
        .unwrap_or_default();

    let with_scheme = if spec.contains("://") {
        spec
    } else {
        format!("http://{spec}")
    };
    let uri = with_scheme.parse::<Uri>().map_err(|_| ())?;
    let protocol = ProxyProtocol::try_from(uri.scheme_str().unwrap_or("http")).map_err(|_| ())?;
    let authority = uri.authority().ok_or(())?;
    // curl defaults every proxy protocol to 1080; ureq defaults HTTP(S) to 80/443.
    let mut builder = Proxy::builder(protocol)
        .host(authority.host())
        .port(authority.port_u16().unwrap_or(1080));
    // FIXME(ureq): URL credentials should be decoded by ureq. Its current ProxyBuilder
    // interpolates credentials into a URI, then its accessors parse them again. Decode
    // locally and verify the resulting endpoint and authentication below until upstream
    // can preserve credentials independently of URI syntax.
    let credentials = if let Some((userinfo, _)) = authority.as_str().rsplit_once('@') {
        // Split before decoding; a literal '+' stays '+' as required for URI userinfo.
        let (username, password) = userinfo
            .split_once(':')
            .map_or((userinfo, None), |(username, password)| {
                (username, Some(password))
            });
        let username = percent_decode_str(username)
            .decode_utf8()
            .map_err(|_| ())?
            .into_owned();
        let password = password
            .map(|password| {
                percent_decode_str(password)
                    .decode_utf8()
                    .map(|value| value.into_owned())
                    .map_err(|_| ())
            })
            .transpose()?;
        builder = builder.username(&username);
        if let Some(password) = &password {
            builder = builder.password(password);
        }
        Some((username, password))
    } else {
        None
    };
    // curl trims each entry and treats `example.com` and `.example.com` alike, matching the
    // domain and every subdomain. ureq matches a bare name exactly and a dotted one as a suffix
    // only, so each entry is registered in both forms.
    for entry in no_proxy
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        if entry == "*" {
            builder = builder.no_proxy(entry);
            continue;
        }
        let domain = entry.trim_start_matches('.');
        builder = builder.no_proxy(domain).no_proxy(&format!(".{domain}"));
    }
    let proxy = builder.build().map_err(|_| ())?;
    // ureq rebuilds a URI from the decoded credentials. Refuse credentials that change
    // its authority (for example an encoded '/') instead of connecting to a different host.
    if proxy.host() != authority.host() || proxy.port() != authority.port_u16().unwrap_or(1080) {
        return Err(());
    }
    if let Some((username, password)) = credentials {
        let matches = match protocol {
            ProxyProtocol::Http | ProxyProtocol::Https => {
                // HTTP Basic joins both fields with ':', so a ':' in the password can
                // still produce correct authentication even when ureq reparses it.
                // A ':' in the username cannot be represented by HTTP Basic.
                !username.contains(':')
                    && format!("{username}:{}", password.as_deref().unwrap_or_default())
                        == format!(
                            "{}:{}",
                            proxy.username().unwrap_or_default(),
                            proxy.password().unwrap_or_default(),
                        )
            }
            // SOCKS5 transmits these fields separately: recombining them would conceal
            // a password such as 'p:ss' becoming username 'user:p' and password 'ss'.
            _ => {
                proxy.username() == Some(username.as_str())
                    && proxy.password() == password.as_deref()
            }
        };
        if !matches {
            return Err(());
        }
    }
    Ok(Some(proxy))
}

/// Extracts `(sha, reference)` pairs from a git smart-HTTP advertisement.
///
/// The advertisement is scanned for `<40 hex digits> refs/...` rather than decoded as pkt-lines.
/// Only the ref names matter here, and scanning tolerates the framing variations that appear
/// around the first ref: a pkt-line length prefix, the `service=git-upload-pack` header, and the
/// NUL-separated capability list. A 40-digit hexadecimal prefix is specific enough that no other
/// part of the advertisement matches it.
fn parse_remote_refs(advertisement: &[u8]) -> Vec<(String, String)> {
    let mut refs = Vec::new();
    let marker = b" refs/";

    for marker_start in find_all(advertisement, marker) {
        // A SHA-1 object name is 40 hexadecimal digits, and it ends where the marker's space
        // begins.
        if marker_start < 40
            || !advertisement[marker_start - 40..marker_start]
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            continue;
        }
        // Skip the marker's leading space; the reference itself starts at `refs/`.
        let reference_start = marker_start + 1;
        // A ref name ends at the next separator: a newline between entries, or the NUL that
        // introduces the capability list on the first one.
        let reference_end = advertisement[reference_start..]
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
            .map_or(advertisement.len(), |offset| reference_start + offset);
        let Ok(reference) = std::str::from_utf8(&advertisement[reference_start..reference_end])
        else {
            continue;
        };
        let Ok(sha) = std::str::from_utf8(&advertisement[marker_start - 40..marker_start]) else {
            continue;
        };
        refs.push((sha.to_owned(), reference.to_owned()));
    }

    refs
}

fn find_all<'a>(haystack: &'a [u8], needle: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(move |(index, window)| (window == needle).then_some(index))
}

#[cfg(test)]
mod tests {
    use super::{https_proxy, parse_remote_refs};
    use base64::Engine;
    use base64::prelude::BASE64_STANDARD;
    use std::sync::{Arc, Mutex};
    use ureq::config::Config;
    use ureq::http::Uri;
    use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
    use ureq::unversioned::transport::{
        Buffers, ConnectProxyConnector, ConnectionDetails, Connector, LazyBuffers, NextTimeout,
        Transport,
    };
    use ureq::{Agent, Error, ProxyProtocol};

    fn proxy_for(vars: &[(&str, &str)]) -> Result<Option<ureq::Proxy>, ()> {
        https_proxy(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    fn bypasses(proxy: &ureq::Proxy, host: &str) -> bool {
        proxy.is_no_proxy(&format!("https://{host}/").parse::<Uri>().unwrap())
    }

    #[test]
    fn proxy_selection_follows_curl_precedence_for_https() {
        assert!(proxy_for(&[]).unwrap().is_none());
        // curl never uses http_proxy for an HTTPS URL.
        assert!(
            proxy_for(&[("http_proxy", "h:1"), ("HTTP_PROXY", "h:1")])
                .unwrap()
                .is_none()
        );
        assert!(proxy_for(&[("https_proxy", "")]).unwrap().is_none());

        let host = |vars: &[(&str, &str)]| proxy_for(vars).unwrap().unwrap().host().to_owned();
        assert_eq!(
            host(&[("https_proxy", "lower"), ("HTTPS_PROXY", "upper")]),
            "lower"
        );
        assert_eq!(
            host(&[("HTTPS_PROXY", "specific"), ("all_proxy", "all")]),
            "specific"
        );
        assert_eq!(host(&[("https_proxy", ""), ("ALL_PROXY", "all")]), "all");
    }

    #[test]
    fn proxy_specification_matches_curl_defaults() {
        let proxy = proxy_for(&[("https_proxy", "user:p%40ss@proxy.example:3128")])
            .unwrap()
            .unwrap();
        assert_eq!(proxy.protocol(), ProxyProtocol::Http);
        assert_eq!(proxy.host(), "proxy.example");
        assert_eq!(proxy.port(), 3128);
        assert_eq!(proxy.username(), Some("user"));
        assert_eq!(proxy.password(), Some("p@ss"));

        let socks = proxy_for(&[("ALL_PROXY", "socks5h://gateway:1080")])
            .unwrap()
            .unwrap();
        assert_eq!(socks.protocol(), ProxyProtocol::Socks5h);
        assert_eq!(socks.port(), 1080);

        assert!(proxy_for(&[("https_proxy", "ftp://proxy.example")]).is_err());
    }

    #[test]
    fn proxy_ports_follow_curl_defaults_and_explicit_settings() {
        for scheme in [
            "",
            "http://",
            "https://",
            "socks4://",
            "socks4a://",
            "socks5://",
            "socks5h://",
        ] {
            for (suffix, expected) in [("", 1080), (":3128", 3128), (":80", 80), (":443", 443)] {
                let spec = format!("{scheme}proxy.example{suffix}");
                for variable in ["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"] {
                    let proxy = proxy_for(&[(variable, &spec)]).unwrap().unwrap();
                    assert_eq!(proxy.port(), expected, "{variable}={spec}");
                }
            }
        }
    }

    #[test]
    fn connect_authentication_uses_decoded_credentials() {
        for (userinfo, expected) in [
            ("user:p%40ss", "user:p@ss"),
            ("u%40ser:p%25ss", "u@ser:p%ss"),
            ("user:p%3ass", "user:p:ss"),
            ("user:p+ss", "user:p+ss"),
            ("user:p%2540ss", "user:p%40ss"),
            ("u%40ser", "u@ser:"),
        ] {
            let spec = format!("http://{userinfo}@proxy.example:3128");
            let proxy = proxy_for(&[("https_proxy", &spec)]).unwrap().unwrap();
            let request = Arc::new(Mutex::new(Vec::new()));
            let connector = ConnectProxyConnector::default().chain(ProxyFixture(request.clone()));
            let agent = Agent::with_parts(
                Agent::config_builder().proxy(Some(proxy)).build(),
                connector,
                ProxyFixture(request.clone()),
            );
            // A fixed 407 ends the CONNECT exchange before TLS. The real ureq connector
            // generates the authentication header; neither sockets nor DNS are involved.
            assert!(matches!(
                agent.get("https://github.com/").call(),
                Err(Error::ConnectProxyFailed(_))
            ));
            let request = String::from_utf8(request.lock().unwrap().clone()).unwrap();
            assert!(request.starts_with("CONNECT github.com:443 HTTP/1.1\r\n"));
            let credentials = request
                .lines()
                .find_map(|line| line.strip_prefix("Proxy-Authorization: Basic "))
                .expect("CONNECT must carry proxy authentication");
            assert_eq!(
                BASE64_STANDARD.decode(credentials).unwrap(),
                expected.as_bytes()
            );
        }
    }

    #[test]
    fn proxy_credentials_reject_invalid_utf8() {
        for userinfo in ["%ff:pass", "user:%ff"] {
            let spec = format!("http://{userinfo}@proxy.example:3128");
            assert!(proxy_for(&[("https_proxy", &spec)]).is_err());
        }
    }

    #[test]
    fn decoded_credentials_cannot_change_the_proxy_endpoint() {
        for scheme in ["http", "https", "socks5", "socks5h"] {
            for userinfo in [
                "user:p%2fss",
                "user:p%3fss",
                "user:p%23ss",
                "user:p%20ss",
                "user:p%C3%A9ss",
            ] {
                let spec = format!("{scheme}://{userinfo}@proxy.example:3128");
                assert!(proxy_for(&[("https_proxy", &spec)]).is_err(), "{spec}");
            }
        }
    }

    #[test]
    fn socks_credentials_preserve_each_decoded_field() {
        for scheme in ["socks5", "socks5h"] {
            for (userinfo, username, password) in [
                ("u%40ser:p%40ss", "u@ser", Some("p@ss")),
                ("user:p%25ss", "user", Some("p%ss")),
                ("user:p%2540ss", "user", Some("p%40ss")),
                ("user:p+ss", "user", Some("p+ss")),
                ("u%40ser", "u@ser", None),
                ("user:", "user", Some("")),
                ("u%3Aser:pass", "u:ser", Some("pass")),
            ] {
                let spec = format!("{scheme}://{userinfo}@proxy.example:3128");
                let proxy = proxy_for(&[("https_proxy", &spec)]).unwrap().unwrap();
                assert_eq!(proxy.username(), Some(username));
                assert_eq!(proxy.password(), password);
            }
            for userinfo in ["user:p%3Ass", "user:p:ss", "u%3Aser"] {
                let spec = format!("{scheme}://{userinfo}@proxy.example:3128");
                assert!(proxy_for(&[("https_proxy", &spec)]).is_err(), "{spec}");
            }
        }
    }

    #[test]
    fn basic_auth_rejects_a_colon_in_the_decoded_username() {
        for scheme in ["http", "https"] {
            let spec = format!("{scheme}://u%3Aser:pass@proxy.example:3128");
            assert!(proxy_for(&[("https_proxy", &spec)]).is_err());
        }
    }

    /// Supplies the proxy exchange in memory, while ureq constructs the CONNECT request.
    #[derive(Debug)]
    struct ProxyFixture(Arc<Mutex<Vec<u8>>>);

    impl Resolver for ProxyFixture {
        fn resolve(
            &self,
            uri: &Uri,
            _: &Config,
            _: NextTimeout,
        ) -> Result<ResolvedSocketAddrs, Error> {
            let mut addresses = self.empty();
            addresses.push(([127, 0, 0, 1], uri.port_u16().unwrap_or(443)).into());
            Ok(addresses)
        }
    }

    impl<In: Transport> Connector<In> for ProxyFixture {
        type Out = Box<dyn Transport>;

        fn connect(
            &self,
            details: &ConnectionDetails,
            chained: Option<In>,
        ) -> Result<Option<Self::Out>, Error> {
            if let Some(transport) = chained {
                return Ok(Some(transport.boxed()));
            }
            assert_eq!(details.uri.host(), Some("proxy.example"));
            assert_eq!(details.uri.port_u16(), Some(3128));
            Ok(Some(
                ProxyTransport {
                    request: self.0.clone(),
                    buffers: LazyBuffers::new(1024, 1024),
                }
                .boxed(),
            ))
        }
    }

    #[derive(Debug)]
    struct ProxyTransport {
        request: Arc<Mutex<Vec<u8>>>,
        buffers: LazyBuffers,
    }

    impl Transport for ProxyTransport {
        fn buffers(&mut self) -> &mut dyn Buffers {
            &mut self.buffers
        }

        fn transmit_output(&mut self, amount: usize, _: NextTimeout) -> Result<(), Error> {
            self.request
                .lock()
                .unwrap()
                .extend_from_slice(&self.buffers.output()[..amount]);
            Ok(())
        }

        fn await_input(&mut self, _: NextTimeout) -> Result<bool, Error> {
            let response =
                b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n";
            self.buffers.input_append_buf()[..response.len()].copy_from_slice(response);
            self.buffers.input_appended(response.len());
            Ok(true)
        }

        fn is_open(&mut self) -> bool {
            true
        }
    }

    #[test]
    fn no_proxy_matches_domains_like_curl() {
        let proxy = proxy_for(&[
            ("https_proxy", "proxy.example:3128"),
            ("NO_PROXY", " github.com , .internal,localhost"),
        ])
        .unwrap()
        .unwrap();
        assert!(bypasses(&proxy, "github.com"));
        assert!(bypasses(&proxy, "codeload.github.com"));
        assert!(bypasses(&proxy, "internal"));
        assert!(bypasses(&proxy, "a.b.internal"));
        assert!(bypasses(&proxy, "localhost"));
        assert!(!bypasses(&proxy, "notgithub.com"));
        assert!(!bypasses(&proxy, "objects.githubusercontent.com"));

        let lowercase_wins = proxy_for(&[
            ("https_proxy", "proxy.example"),
            ("no_proxy", "a.example"),
            ("NO_PROXY", "*"),
        ])
        .unwrap()
        .unwrap();
        assert!(!bypasses(&lowercase_wins, "github.com"));

        let all = proxy_for(&[("https_proxy", "proxy.example"), ("no_proxy", "*")])
            .unwrap()
            .unwrap();
        assert!(bypasses(&all, "github.com"));
    }

    #[test]
    fn parses_unpeeled_tag_refs() {
        let refs = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa refs/tags/v1.2.3\0capability\n\
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb refs/tags/v2.0.0^{}\n\
cccccccccccccccccccccccccccccccccccccccc refs/tags/v2.0.0-rc.1\n";

        assert_eq!(
            parse_remote_refs(refs),
            [
                (
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                    "refs/tags/v1.2.3".to_owned()
                ),
                (
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                    "refs/tags/v2.0.0^{}".to_owned()
                ),
                (
                    "cccccccccccccccccccccccccccccccccccccccc".to_owned(),
                    "refs/tags/v2.0.0-rc.1".to_owned()
                ),
            ]
        );
    }
}
