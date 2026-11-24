# Oculus App Makefile
# Provides unified verification targets for CI/CD

.PHONY: verify check-sdk check-python-artifacts check-placeholders analyze test

# SW-098: a wrapper on PATH answered `flutter analyze` / `flutter test` with
# exit 0 without running anything. Point FLUTTER at a real SDK
# (e.g. make FLUTTER=~/flutter_stable/bin/flutter); check-sdk refuses any
# binary that cannot report a real framework/Dart version.
FLUTTER ?= flutter

verify: check-sdk check-python-artifacts check-placeholders analyze test

check-sdk:
	@$(FLUTTER) --version --machine 2>/dev/null | grep -q '"dartSdkVersion"' || { \
		echo "ERROR: '$(FLUTTER)' is not a real Flutter SDK (no version report)."; \
		echo "       Run: make FLUTTER=/path/to/flutter/bin/flutter verify"; \
		exit 1; }
	@$(FLUTTER) --version | head -1

check-python-artifacts:
	@echo "Checking for Python artifacts in Flutter lib..."
	@if [ -f "lib/services/__init__.py" ]; then \
		echo "ERROR: Python artifact still present: lib/services/__init.py"; \
		exit 1; \
	fi
	@echo "No Python artifacts found."

check-placeholders:
	@echo "Checking for PlaceholderScreen references..."
	@if grep -r "PlaceholderScreen" "lib/" "test/" 2>/dev/null; then \
		echo "ERROR: PlaceholderScreen references found."; \
		exit 1; \
	fi
	@echo "No PlaceholderScreen references found."

analyze: check-sdk
	@echo "Running flutter analyze..."
	$(FLUTTER) analyze

test: check-sdk
	@echo "Running flutter test..."
	$(FLUTTER) test
