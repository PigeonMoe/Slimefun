# Slimefun for Horizon 26.3

本分支从 PigeonMoe Slimefun 5.0.1 的 `26.2` 分支（`f6d16b6`）移植，保留原有 GPLv3 许可证。已发布版本为服务端 WebAssembly 预览，提供部分多方块机器。全量 1:1 移植仍在开发。

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

## 本地开发增量

`0.1.0-dev.2` 加入了 Ore Crusher、Compressor、碳块配方和四项研究。Ore Crusher 默认启用原版的矿物翻倍。可在 `slimefun-server.toml` 将 `double-ores` 设置为 `false`，服务端会在读取该文件时应用设置。Compressor 按原版在第 0、20 和 40 个刻播放活塞声音，并在第 60 个刻交付产物。四种机器优先将产物写入上、北、东、南、西方向中第一个容量足够的已标记输出箱，其次尝试发射器。压缩机交付时库存满会尝试在机器位置掉落余量。

输出箱沿用原版 3×3 配方和 20 级研究。当时的版本需要已导入的 Slimefun 铅锭。标记随放置后的容器保存。

使用 `scripts/runtime-smoke.py` 的 `--export-horizon-catalog` 可在独立 Paper 测试服导出 `probe-horizon-catalog.json`。输出记录该 Paper 实例中已注册的物品、配方、研究、原始物品 NBT，以及多方块机器的配方顺序。它用作对照样本。

以下命令从 Paper 的 `data-storage/Slimefun/Players` 生成研究导入文件。Python 环境需要 PyYAML。目标目录位于对应 Horizon 世界内。

```sh
python3 horizon/tools/import-research.py probe-horizon-catalog.json \
  /path/to/paper/data-storage/Slimefun/Players \
  /path/to/horizon-world/horizon-mod-data/slimefun/research-import
```

匹配同一 UUID 的玩家进入 Horizon 后会合并研究记录。导入保留已有研究和其他持久数据，重复导入不会扣除经验。该工具支持旧 YAML 的研究项，背包、航点、世界方块数据及 PM-Sync 数据仍待移植。

### 从砾石到输出箱

本地开发版 `0.1.0-dev.3` 补齐了淘金、洗矿、冶炼和输出箱的生存获取链。该版本需要 PigeonMoe Horizon Fork 提交 `d2679a263841152d461e921fba39007e9ba4d3a1` 的 Mod API，用于淘金时受保护的方块移除和箱子掉落读取。

1. 在增强工作台制作淘金盘。点砾石会走服务器的破坏许可检查，允许后移除砾石并产出单个掉落物。默认权重沿用 Paper 分支，筛过的矿石为 35%。
2. 在空炼药锅上放木栅栏，再在栅栏上方放发射器。把筛过的矿石放入发射器，点栅栏洗出九种粉末中的一种和石块副产物。铅粉占九种粉末之一。
3. 简易冶炼炉从上到下使用木栅栏、发射器和火。发射器两侧用红砖。它处理单种粉末配方，每次耗掉火。常规冶炼炉改用下界砖栅栏和下界砖，支持更多输入组合与合金。两种炉都能将铅粉冶炼成铅锭。
4. 研究输出箱，并在增强工作台使用五个铅锭、漏斗和箱子制作。玩家拆除带标记的输出箱时，服务端会在原本的箱子掉落上恢复 Slimefun ID。

四项设置保存在该 Horizon 存档的 `slimefun-server.toml`。默认值为 `double-ores = true`、`legacy-ore-washer = false`、`fire-breaking-chance = 34` 和 `output-chest-enabled = true`。最后一项控制该存档中的输出箱配方和产物路由，维度级覆盖仍待补齐。

冶炼炉点火室自动复燃、非玩家移除导致的输出箱标识恢复及其他配方行为仍待移植。开发版以本地 Fork 构建和原版代码为校验基线，真实客户端的整条生产路线仍需验收。
