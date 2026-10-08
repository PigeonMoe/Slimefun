# Slimefun — PigeonMoe 社区维护分支

基于 [Slimefun/Slimefun4](https://github.com/Slimefun/Slimefun4) 的 `experimental` 分支继续维护。这是独立的社区 Fork，未获得原项目的官方交接身份。原作者和社区贡献者的版权、GPLv3 许可证保持不变。

## 版本分支

| 分支 | 项目版本 | Minecraft / 服务端 | Java |
| --- | --- | --- | --- |
| `experimental` | 上游基线 | 保留上游原始内容 | 见上游 |
| `1.21.11` | Slimefun4 4.9.1 | Paper 1.21.11 | 21+ |
| `26.2` | Slimefun5 开发线 | Paper 26.2 | 25+ |

每个 Minecraft 版本独立维护。不要把新版本的世界降级，也不要在现有生产存档上直接测试开发构建。支持范围与验收记录见 [兼容性记录](docs/COMPATIBILITY.md)。

## 构建

```sh
mvn -B -ntp clean verify
```

本分支使用 JDK 21。插件、对应源码包和内嵌 GPLv3 许可证位于 `target/`。仓库名、Bukkit 插件名和数据目录仍为 `Slimefun`；保留原 Java 包名、物品 ID 和持久化键。`experimental` 已改变 `SlimefunItemStack` 等 API，旧附属插件需要单独核对，不能推定全部兼容。

## 本地运行验证（当前待用户提供核心并授权启动）

```sh
python3 scripts/runtime-smoke.py --minecraft-version 1.21.11 \
  --jar 'target/Slimefun v4.9.1-UNOFFICIAL.jar' \
  --paper-jar /path/to/your/paper-1.21.11.jar --accept-eula
```

运行前阅读并接受 [Minecraft EULA](https://aka.ms/MinecraftEULA)。脚本使用 `target/runtime-smoke/` 内的独立测试存档和随机本机端口，使用用户提供的 Paper 核心并记录其校验和，检查插件启用、核心物品与 ID 序列化、所有头颅纹理和放置后的纹理更新，并在完成后关闭测试服。结果和日志保留在该目录。它不等同于玩家登录、完整机器运行或附属插件实测。

## 语言与更新

在 `plugins/Slimefun/config.yml` 设置 `options.language: zh-CN` 或 `zh-TW`，并开启 `options.enable-translations`。现有消息、分类、配方说明、研究和资源翻译来自上游社区；Slimefun5 继续扩展物品本地化。

构建不会自动下载原项目的 Slimefun4 更新。请从本仓库的 Actions 构建或 Releases 获取对应版本。问题请提交到 [本仓库 Issues](https://github.com/PigeonMoe/Slimefun/issues)。上游专用的发布、通知、自动合并工作流保存在 `docs/upstream-workflows/`，不在此 Fork 执行。

原始项目介绍见 [上游 README](docs/UPSTREAM-README.md)。许可证：[GNU GPLv3](LICENSE)。
