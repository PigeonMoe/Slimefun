# Slimefun for Horizon 26.3

本分支从 PigeonMoe Slimefun 5.0.1 的 `26.2` 分支（`f6d16b6`）移植，保留原有 GPLv3 许可证。当前版本为服务端 WebAssembly 预览，提供部分多方块机器。全量 1:1 移植仍在开发。

## 安装

1. 下载本仓库预发布中的 `slimefun.hmod` 和 `horizon-wasm-std.wasm`，使用 `SHA256SUMS` 核对文件。服务端使用与 `dc215e79e27d1cf55bb041a2c1bf2c02a701cd74` 一致的 Mod API 布局，并将编译工具链为 Rust 1.99.1 的 `horizon-wasm-std.wasm` 放在游戏引擎可执行文件旁。这个版本使用普通 Mod API hooks，支持常规服务端引擎构建。
2. 将文件放进服务端游戏目录的 `horizon-mods/`，在该游戏目录的 `horizon-mods.txt` 添加 `mod.slimefun:true`，然后重启。
3. 如使用 PigeonMoe Horizon Panel，将 [slimefun.toml](panel-plugins/slimefun.toml) 放进服务端实例目录的 `panel-plugins/`。启用面板后，该条目调用固定的 `slimefun status` 查询命令。

## 游戏内

- `/sf guide` 显示两种机器的摆放方式，`/sf recipe <物品>` 显示已移植配方，`/sf status` 显示覆盖范围。
- 将橡木栅栏放在朝上的发射器上方，装入素材后对栅栏点右键。磨石包含原分支的 26 条默认配方，包括烈焰棒、下界疣、末影之眼和小麦。
- 将工作台放在发射器上方，在发射器内排列 3×3 配方后对工作台点右键。增强工作台包含四条魔法块及末影块升级配方和一条海带饼干配方。
- `/sf research lumps` 和 `/sf research kelp_cookie` 分别花费 3 级和 4 级经验。创造模式可免费使用。研究解锁保存在 Horizon 玩家的持久数据中。

两台机器都将产物放回发射器。锁定的发射器拒绝加工，产物空间不足时回滚库存变更。支持的自定义物品使用原来的 `slimefun:slimefun_item` 持久键和原始 ID。与 PurtmarsArea 共用的服务器应采用具备发射器坐标保护的 Horizon 分支。

## 覆盖范围

| 已实现 | 尚待移植或验证 |
| --- | --- |
| 磨石、增强工作台的上述配方 | 其余增强工作台配方、输出箱、其他机器、能源与货运网络 |
| 上述物品的 Paper 持久键和原始 ID | 完整物品目录、动态物品行为、指南 GUI、多语言 |
| 两项研究及 Horizon 存档记录 | Paper 存档和数据库迁移、PM-Sync 跨服同步 |
| Horizon Mod API | Bukkit/Paper 的 Java 插件 API 和 Slimefun 附属插件 |

Horizon 的直接 JAR 引用只加载资源和数据。完整 Java 附属插件兼容需要 Bukkit/Paper 运行层，原有游戏功能也需要逐项移植和验收。

请在独立测试存档中验证。本预览未通过真实客户端、完整生产存档或 Paper 数据迁移的端到端验收。

## 构建与检查

准备 Rust 和 [PigeonMoe Horizon 服务端源码](https://github.com/PigeonMoe/horizon)。源码 checkout 需包含 `horizon/Cargo.toml` 钉住的 Mod API 提交。Horizon 的打包工具会使用其指定的 WebAssembly 编译工具链。

```sh
./horizon/build.sh ../horizon test
./horizon/build.sh ../horizon build
```

构建产物位于 `horizon/target/horizon-mod/slimefun.hmod`。如需自行生成匹配的 Wasm 标准库，在 Horizon 服务端源码目录用该目录的打包工具运行以下指令。预发布中的标准库也使用这条命令生成。

```sh
cargo build --locked -p cargo-horizon-mod
target/debug/cargo-horizon-mod wasm-std target/horizon-wasm-std.wasm
```

Java 版源码和原有构建指令位于仓库根目录。
