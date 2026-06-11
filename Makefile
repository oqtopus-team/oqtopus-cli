SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

.PHONY: install diff-backend-configs docs-lint docs-build docs-serve help

install: ## Install dependencies and configure git hooks and commit template
	@uv sync --all-groups
	@if [ -d .git ]; then \
		git config --local commit.template .gitmessage; \
	fi
	@chmod +x scripts/diff-backend-configs.sh

diff-backend-configs: ## Compare backend template configs against upstream repositories
	@scripts/diff-backend-configs.sh

docs-lint: ## Run documentation linting
	@uv run pymarkdownlnt scan docs

docs-build: ## Build documentation
	@uv run mkdocs build

docs-serve: ## Serve documentation locally
	@uv run mkdocs serve

help: ## Show help message
	@echo "Usage: make [target]"
	@echo ""
	@echo "Available targets:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(filter-out .env,$(MAKEFILE_LIST)) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'
