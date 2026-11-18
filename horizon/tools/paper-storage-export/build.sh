#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
set -euo pipefail

if [[ $# -ne 1 || -z ${PAPER_EXPORT_CLASSPATH:-} ]]; then
    echo "Usage: PAPER_EXPORT_CLASSPATH=<Paper API and its libraries> $0 /path/to/exporter.jar|--test" >&2
    exit 2
fi

source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
javac --release 25 -cp "$PAPER_EXPORT_CLASSPATH" -d "$work" "$source_dir/PaperStorageExporter.java"
if [[ $1 == --test ]]; then
    classpath="$work:$PAPER_EXPORT_CLASSPATH"
    javac --release 25 -cp "$classpath" -d "$work" "$source_dir/PaperStorageExporterTest.java"
    java -cp "$classpath" moe.pigeon.horizon.migration.PaperStorageExporterTest
    exit
fi
jar --create --file "$1" -C "$work" . -C "$source_dir" plugin.yml
