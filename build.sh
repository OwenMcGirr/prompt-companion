#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")" && pwd)"
output_dir="${1:-$(dirname "$project_dir")}"
build_dir="${PROMPT_COMPANION_BUILD_DIR:-$project_dir/.build}"
swift build --package-path "$project_dir" --scratch-path "$build_dir" -c release
binary_dir="$(swift build --package-path "$project_dir" --scratch-path "$build_dir" -c release --show-bin-path)"
app_dir="$output_dir/Prompt Companion.app"
mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
cp "$binary_dir/PromptCompanion" "$app_dir/Contents/MacOS/PromptCompanion"
cp "$project_dir/Resources/Info.plist" "$app_dir/Contents/Info.plist"
if [ -f "$project_dir/Resources/AppIcon.icns" ]; then
    cp "$project_dir/Resources/AppIcon.icns" "$app_dir/Contents/Resources/AppIcon.icns"
fi
codesign --force --sign - --identifier local.promptcompanion.mac "$app_dir"
echo "Built: $app_dir"
