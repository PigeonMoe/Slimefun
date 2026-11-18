#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
set -euo pipefail

if [[ $# -ne 1 || -z ${PAPER_EXPORT_CLASSPATH:-} ]]; then
    echo "Usage: PAPER_EXPORT_CLASSPATH=<patched Paper 26.2 JAR and its libraries> $0 /path/to/exporter.jar|--test" >&2
    exit 2
fi

source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
paper_sources="$source_dir/../../../src/main/java/io/github/thebusybiscuit/slimefun4/core/services/sync"
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
javac --release 25 -cp "$PAPER_EXPORT_CLASSPATH" -d "$work" \
    "$source_dir/PaperStorageExporter.java" "$source_dir/PaperLegacyItem.java" \
    "$source_dir/PaperSyncedProfiles.java" "$paper_sources/ProfileSnapshot.java"
if [[ $1 == --test ]]; then
    classpath="$work:$PAPER_EXPORT_CLASSPATH"
    javac --release 25 -cp "$classpath" -d "$work" \
        "$source_dir/PaperStorageExporterTest.java" "$source_dir/PaperStorageExporterLegacyTest.java" \
        "$source_dir/PaperSyncedProfilesFixture.java"
    (
        cd -- "$work"
        java --enable-native-access=ALL-UNNAMED -cp "$classpath" \
            moe.pigeon.horizon.migration.PaperStorageExporterTest
        java --enable-native-access=ALL-UNNAMED -cp "$classpath" \
            moe.pigeon.horizon.migration.PaperStorageExporterLegacyTest "$source_dir/fixtures"
    )
    exit
fi
jar --create --manifest "$source_dir/manifest.mf" --file "$1" -C "$work" . -C "$source_dir" plugin.yml
