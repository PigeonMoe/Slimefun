#!/usr/bin/env python3
"""Run bounded integration probes on loopback-only Paper in target/runtime-smoke.
Requires an explicit --accept-eula flag for the disposable test server.
No production server, world, port or plugin directory is used.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--minecraft-version', required=True)
    parser.add_argument('--jar', required=True, type=Path)
    parser.add_argument('--paper-jar', required=True, type=Path, help='User-provided Paper server core; never downloaded automatically')
    parser.add_argument('--java-home', default=os.environ.get('JAVA_HOME'), required=not os.environ.get('JAVA_HOME'))
    parser.add_argument('--accept-eula', action='store_true')
    args = parser.parse_args()
    if not args.accept_eula:
        parser.error('Pass --accept-eula after accepting https://aka.ms/MinecraftEULA for this test server')
    version = args.minecraft_version
    jar = args.jar.resolve()
    if not jar.is_file():
        parser.error(f'JAR does not exist: {jar}')
    runtime = ROOT / 'target' / 'runtime-smoke' / version
    runtime.mkdir(parents=True, exist_ok=True)
    paper = runtime / 'paper.jar'
    source_paper = args.paper_jar.resolve()
    if not source_paper.is_file():
        parser.error(f'Paper core does not exist: {source_paper}')
    if source_paper != paper.resolve():
        shutil.copy2(source_paper, paper)
    plugins = runtime / 'plugins'
    plugins.mkdir(exist_ok=True)
    shutil.copy2(jar, plugins / 'Slimefun.jar')
    java_bin = Path(args.java_home) / 'bin'
    classpath_file = ROOT / 'target' / 'smoke-classpath.txt'
    env = dict(os.environ, JAVA_HOME=args.java_home, PATH=str(java_bin) + os.pathsep + os.environ['PATH'])
    subprocess.run(['mvn', '-B', '-ntp', 'dependency:build-classpath', f'-Dmdep.outputFile={classpath_file}'],
                   cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL)
    classes = runtime / 'probe-classes'
    classes.mkdir(exist_ok=True)
    subprocess.run([str(java_bin / 'javac'), '--release', '21', '-proc:none', '-cp',
                    str(jar) + os.pathsep + classpath_file.read_text().strip(), '-d', str(classes),
                    str(ROOT / 'scripts/runtime-smoke/RuntimeSmoke.java')], check=True)
    descriptor = 'name: SlimefunRuntimeSmoke\nversion: 1.0\nmain: dev.pigeonmoe.slimefun.testing.RuntimeSmoke\napi-version: "1.21.11"\ndepend: [Slimefun]\n'
    with zipfile.ZipFile(plugins / 'RuntimeSmoke.jar', 'w', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('plugin.yml', descriptor)
        for file in classes.rglob('*.class'):
            archive.write(file, file.relative_to(classes))
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    (runtime / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nlevel-type=minecraft:flat\ngenerator-settings={{"layers":[{{"block":"minecraft:bedrock","height":1}},{{"block":"minecraft:dirt","height":2}},{{"block":"minecraft:grass_block","height":1}}],"biome":"minecraft:plains"}}\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\nenable-rcon=false\nenable-query=false\n')
    (runtime / 'eula.txt').write_text('eula=true\n')
    slimefun = plugins / 'Slimefun'
    slimefun.mkdir(exist_ok=True)
    (slimefun / 'config.yml').write_text('options:\n  auto-update: false\n  language: en\n  enable-translations: true\nmetrics:\n  auto-update: false\n  analytics: false\n')
    (plugins / 'bStats').mkdir(exist_ok=True)
    (plugins / 'bStats' / 'config.yml').write_text('enabled: false\n')
    log = runtime / 'console.log'
    with log.open('w') as output:
        process = subprocess.Popen([str(java_bin / 'java'), '-Xms256M', '-Xmx1G',
                                    '-Dpaper.disablePluginRemapping=true', '-jar', str(paper), '--nogui'],
                                   cwd=runtime, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT)
        try:
            process.wait(timeout=240)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            process.stdin.write(b'stop\n')
            process.stdin.flush()
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            raise
    content = log.read_text()
    errors = [line for line in content.splitlines() if 'SLIMEFUN_SMOKE_FAILED' in line
              or any(prefix in line for prefix in ('[Slimefun]', '[dough:')) and ('ERROR' in line or 'SEVERE' in line)]
    success = process.returncode == 0 and 'SLIMEFUN_SMOKE_OK' in content and not errors
    result = {'minecraft': version, 'paper_sha256': hashlib.sha256(paper.read_bytes()).hexdigest(),
              'plugin_sha256': hashlib.sha256(jar.read_bytes()).hexdigest(), 'success': success,
              'log': str(log), 'errors': errors,
              'probes': [line for line in content.splitlines() if 'SLIMEFUN_SMOKE_' in line]}
    (runtime / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
    if not success:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
