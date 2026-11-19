# Slimefun for Horizon 26.3

本分支从 PigeonMoe Slimefun 5.0.1 的 `26.2` 分支（`f6d16b6`）移植，保留原有 GPLv3 许可证。已发布版本为服务端 WebAssembly 预览，提供部分多方块机器。全量 1:1 移植仍在开发。

## 安装

1. 从同一预发布下载 `slimefun.hmod` 和 `horizon-wasm-std.wasm`，使用 `SHA256SUMS` 核对文件。使用该预发布标明的 PigeonMoe Horizon Fork 提交构建服务端，将配套的 `horizon-wasm-std.wasm` 放在游戏引擎可执行文件旁。
2. 将文件放进服务端游戏目录的 `horizon-mods/`，在该游戏目录的 `horizon-mods.txt` 添加 `mod.slimefun:true`，然后重启。
3. 如使用 PigeonMoe Horizon Panel，将 [slimefun.toml](panel-plugins/slimefun.toml) 放进服务端实例目录的 `panel-plugins/`。启用面板后，该条目调用固定的 `slimefun status` 查询命令。

## 游戏内

- `/sf guide` 显示已移植机器的摆放与使用方式，`/sf recipe <物品>` 显示配方，`/sf status` 显示覆盖范围。
- 将橡木栅栏放在朝上的发射器上方，装入素材后对栅栏点右键。磨石包含原分支的 26 条默认配方，包括烈焰棒、下界疣、末影之眼和小麦。
- 将工作台放在发射器上方，在发射器内排列 3×3 配方后对工作台点右键。增强工作台包含四条魔法块及末影块升级配方和一条海带饼干配方。
- `/sf research lumps` 和 `/sf research kelp_cookie` 分别花费 3 级和 4 级经验。创造模式可免费使用。研究解锁保存在 Horizon 玩家的持久数据中。

磨石与增强工作台优先使用已标记的输出箱，其次使用发射器。受保护的发射器会检查领地权限，产物空间不足时回滚库存变更。支持的自定义物品使用原来的 `slimefun:slimefun_item` 持久键和原始 ID。

## 覆盖范围

| 已实现 | 尚待移植或验证 |
| --- | --- |
| 磨石、增强工作台、输出箱、生存矿物处理链及多类电动机器 | 其余机器、物品行为、指南 GUI 与多语言 |
| 太阳能、燃料发电、反应堆、电量缓冲、货运节点与过滤 | 网络配置细节及各路线的真实客户端验收 |
| 研究、已支持机器菜单、原油档案和普通及特殊背包的 Paper 存档恢复 | 全部 Paper 数据类型、共享库存读取与 PM-Sync 跨服同步 |
| Horizon Mod API | Bukkit/Paper 的 Java 插件 API 和 Slimefun 附属插件 |

Horizon 的直接 JAR 引用只加载资源和数据。完整 Java 附属插件兼容需要 Bukkit/Paper 运行层，原有游戏功能也需要逐项移植和验收。

旧预览已使用实际 26.3 客户端验证部分离线联机功能。本分支的新增功能仍在独立存档进行同服验收。

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

本节描述开发分支的变更。使用当前 PigeonMoe Horizon Fork 构建以取得最新的设备和背包接口。

电炉和电动矿物粉碎机各有三档，电动锭粉碎机有一档，加热压力舱有两档。这些机器通过能源调节器取电，保存输入、输出、剩余进度及缓冲电量。部分原料来源和其他电力机器仍待移植。

流体泵消耗 32 J，从下方水源或相连的熔岩源装桶。充电台读取 Paper 的电量字段与旧版物品说明。手动坩埚按 Paper 的间隔填放液体，每步都会检查原玩家在目标位置的改方块权限。每次网络更新时，运行中的核反应堆可提供 250 J，下界之星反应堆可提供 512 J。反应堆需要四周水和对应的冷却单元，货运通过位于反应堆正上方三个方块的访问端口供料和取出副产物。

`0.1.0-dev.2` 加入了 Ore Crusher、Compressor、碳块配方和四项研究。Ore Crusher 默认启用原版的矿物翻倍。可在 `slimefun-server.toml` 将 `double-ores` 设置为 `false`，服务端会在读取该文件时应用设置。Compressor 按原版在第 0、20 和 40 个刻播放活塞声音，并在第 60 个刻交付产物。四种机器优先将产物写入上、北、东、南、西方向中第一个容量足够的已标记输出箱，其次尝试发射器。压缩机交付时库存满会尝试在机器位置掉落余量。

输出箱沿用原版 3×3 配方和 20 级研究。当时的版本需要已导入的 Slimefun 铅锭。标记随放置后的容器保存。

使用 `scripts/runtime-smoke.py` 的 `--export-horizon-catalog` 可在独立 Paper 测试服导出 `probe-horizon-catalog.json`。输出记录该 Paper 实例中已注册的物品、配方、研究、原始物品 NBT，以及多方块机器的配方顺序。它用作对照样本。

以下命令从 Paper 的 `data-storage/Slimefun/Players` 生成研究导入文件。Python 环境需要 PyYAML。目标目录位于对应 Horizon 世界内。

```sh
python3 horizon/tools/import-research.py probe-horizon-catalog.json \
  /path/to/paper/data-storage/Slimefun/Players \
  /path/to/horizon-world/horizon-mod-data/slimefun/research-import
```

匹配同一 UUID 的玩家进入 Horizon 后会合并研究记录。导入保留已有研究和其他持久数据，重复导入不会扣除经验。下方列出了背包恢复步骤，以及启用 PM-Sync 的服务器如何导出同步档案中的背包和研究。

### 从砾石到输出箱

本地开发版 `0.1.0-dev.3` 补齐了淘金、洗矿、冶炼和输出箱的生存获取链。该版本需要 PigeonMoe Horizon Fork 提交 `d2679a263841152d461e921fba39007e9ba4d3a1` 的 Mod API，用于淘金时受保护的方块移除和箱子掉落读取。

1. 在增强工作台制作淘金盘。点砾石会走服务器的破坏许可检查，允许后移除砾石并产出单个掉落物。默认权重沿用 Paper 分支，筛过的矿石为 35%。
2. 在空炼药锅上放木栅栏，再在栅栏上方放发射器。把筛过的矿石放入发射器，点栅栏洗出九种粉末中的一种和石块副产物。铅粉占九种粉末之一。
3. 简易冶炼炉从上到下使用木栅栏、发射器和火。发射器两侧用红砖。它处理单种粉末配方，每次耗掉火。常规冶炼炉改用下界砖栅栏和下界砖，支持更多输入组合与合金。两种炉都能将铅粉冶炼成铅锭。
4. 研究输出箱，并在增强工作台使用五个铅锭、漏斗和箱子制作。玩家拆除带标记的输出箱时，服务端会在原本的箱子掉落上恢复 Slimefun ID。

四项设置保存在该 Horizon 存档的 `slimefun-server.toml`。默认值为 `double-ores = true`、`legacy-ore-washer = false`、`fire-breaking-chance = 34` 和 `output-chest-enabled = true`。最后一项控制该存档中的输出箱配方和产物路由，维度级覆盖仍待补齐。

冶炼炉点火室自动复燃、非玩家移除导致的输出箱标识恢复及其他配方行为仍待移植。开发版以本地 Fork 构建和原版代码为校验基线，真实客户端的整条生产路线仍需验收。

### 下界淘金盘

本地开发版 `0.1.0-dev.4` 加入下界淘金盘。花费 8 级经验研究 `nether_gold_pan`，在增强工作台用普通淘金盘和五块下界砖制作。右击灵魂沙或灵魂土时会检查方块破坏权限，再按 Paper 默认权重从石英、金粒、下界疣、烈焰粉、荧石粉和恶魂之泪中产出一件物品。可用 `/sf recipe ghast_tear` 等命令查询权重。

自定义掉落权重及 Bukkit 物品生成事件仍待移植。

### Paper 方块存档与基础网络

使用 PyYAML 将 Paper 的 Slimefun 方块元数据和容器档案转换到已迁移的 Horizon 世界。先停止 Paper 服务器，或对同一时间点的快照执行导入。

```sh
python3 horizon/tools/import-block-storage.py \
  /path/to/paper/data-storage/Slimefun \
  /path/to/horizon-world/horizon-mod-data/slimefun/storage-import
```

工具默认识别 `world`、`world_nether` 和 `world_the_end`。使用 `--map-world survival=minecraft:overworld` 指定其他世界名。也可以直接传入 Paper `block-backups` 生成的 ZIP 作为输入。原始 ZIP、`.sfb` 和 `.sfi` 保持原样。导入目录已有不同数据时，工具会保留该目录并报错。使用旧版工具生成的目录需要先备份，再用新版工具重新生成。

二级及以上 OP 可用 `/sf storage` 查看统计，再使用 `/sf storage inspect minecraft:overworld -1 64 -17` 查看具体位置。对于已迁移且仍是普通箱子的输出箱，使用 `/sf storage restore-output minecraft:overworld -1 64 -17` 恢复路由标记，原有箱内物品保持原样。目标需位于当前维度的已加载区块。

`/sf storage restore-inventory minecraft:overworld -1 64 -17` 可恢复已移植的电力机器、货运输入节点和高级货运输出节点的菜单库存。电炉系列和高温压力舱都使用 Paper 的原始输入槽 19、20 与输出槽 24、25。命令保留物品数量，使用每件物品的 DataVersion 升级组件。任何一个槽位无效时整份菜单均不写入。带旧式 `type` 或 `meta` 字段的物品保留在原始档案中，`inspect` 会标明需先由 Paper 重新保存的槽位。

`/sf storage restore-chunk minecraft:overworld -1 64 -17` 按位置顺序恢复指定区块已支持的方块，每批 32 条。命令会列出拒绝恢复的坐标和原因。剩余批次可使用输出中的偏移量继续，未移植的方块仍保存在原档案中。

旧式库存可先用 [Paper 存档导出工具](tools/paper-storage-export/build.sh) 转换。用 JDK 25 在临时目录执行 `java -Dpaperclip.patchonly=true -jar /path/to/paper-26.2.jar`，解包 Paper 26.2 及其依赖。将 `versions/26.2/paper-26.2.jar` 与 `libraries/` 内每个 JAR 的绝对路径用冒号连接，填入 `PAPER_EXPORT_CLASSPATH`。执行 `build.sh /path/to/exporter.jar` 构建，或执行 `build.sh --test` 核验历史 ItemMeta 和失败回滚。解包和测试只加载 Paper 的注册表与物品编码器。

将生成的 JAR 装到 Paper 26.2，在本地控制台执行 `sf-export-inventories paper-snapshot normalized-snapshot`。工具保留旧头颅的所有者 UUID 和旧刷怪蛋中的插件 NBT。遇到无法保留的字段会报告文件与槽位，原快照可用于重试。将 `normalized-snapshot` 作为 Horizon 导入脚本的输入。脚本同时归档 `universal-inventories` 和 `Players/*.yml` 中的背包，保留菜单名、背包所有者 UUID、编号和容量。

启用过 PM-Sync 的 Paper 服务器使用 MariaDB/MySQL 中 `pmsync_state` 的已提交档案。停止源端写入并确认备份一致后，从该权威表的只读副本将以下查询导出为无表头、以制表符分隔的 UTF-8 文件 `pm-snapshots.tsv`。

```sql
SELECT player_uuid, revision, HEX(snapshot)
FROM pmsync_state
WHERE snapshot IS NOT NULL
ORDER BY player_uuid;
```

在装有同一 Paper 导出工具的本地控制台执行 `sf-export-synced-profiles pm-snapshots.tsv normalized-pm`。工具校验 PM-Sync v2 和 Slimefun SFP1 的 SHA-256，再核对玩家 UUID，逐一用 Paper 恢复原槽物品组件。任意记录损坏会阻止整个新目录写入。此命令只读导出的文件，处理 Slimefun 背包和研究字段。仍需按对应迁移流程处理角色、末影箱及其他同步插件的数据。

给方块导入脚本附加 `--player-profiles normalized-pm/Players`，可从同一时间点的方块快照或 ZIP 导入方块，并使用 PM-Sync 导出的背包。该参数指定唯一的玩家档案来源。研究通过 `import-research.py <Paper 配方目录 JSON> normalized-pm/Players <Horizon 研究导入目录>` 转换，保留原命名空间解锁并按原目录映射历史数字 ID。备份、PM-Sync 快照和导出的 Players 文件都应保留以供核对。

二级及以上 OP 可执行 `/sf storage restore-universal TEST_SHARED` 恢复精确名称的共享库存，再执行 `/sf storage open-universal TEST_SHARED` 打开。相同预设的多个菜单会直接访问同一份存档，支持跨维度和重启，重复恢复不会补回已经取走的物品。扩展模组的物理方块权限、无线终端和装饰交互需由对应扩展定义。

旧 `.sfi` 只记录预设和有效槽位，默认用 54 格界面显示确有记录的槽位。若已知原预设的标题、尺寸和库存槽，在恢复前可把布局放在世界内 `horizon-mod-data/slimefun/universal-layout/544553545f534841524544.toml`。文件用预设名的 UTF-8 十六进制命名。布局必须覆盖所有已归档物品，空档案需要原布局。

```toml
preset = "TEST_SHARED"
title = "Shared Inventory"
size = 18
slots = [0, 8, 17]
```

已有 Paper 编号的六档普通背包（9–54 格）可在原物品上恢复。先迁移持有该背包的玩家物品，再由原 UUID 所有者或二级及以上 OP 将背包拿在主手执行 `/sf restore-backpack`。解锁对应研究后，持有者可在任意维度右键访问同一份库存，转交物品后 UUID 与背包编号保持原样。重复恢复会保留当前库存。背包不能装入其他背包或潜影盒。

六档普通背包和布料均可在增强工作台制作。用 `/sf recipe SMALL_BACKPACK` 查询入门配方。空白背包首次打开时会绑定一个新编号，制作与升档也会分配或继承编号。升档保留已有各槽及原所有者，Paper 背包需先恢复才能升档。

已有的 Paper 灵魂绑定背包可用同一命令恢复 36 格库存。灵魂绑定背包和带既有 Soulbound 标记的其他背包会在死亡时留在原槽，保留物品规则和消失诅咒继续由原版处理。右键末影背包会打开当前玩家的原版末影箱，与实际末影箱共用同一份库存。两款背包都在魔法工作台制作。用编织背包合成灵魂绑定背包时，原所有者 UUID、编号、36 格内容和已有 Soulbound 标记随成品保留。Paper 网络同步仍待移植。

在魔法工作台制作古代祭坛和底座。在祭坛同一高度，四个底座各离中心三格，其余四个分别沿对角线在 X、Z 各偏移两格。用 `/sf recipe ESSENCE_OF_AFTERLIFE` 或对应符文 ID 查询配方。把环上八件材料逐件右键放到底座上，手持配方中心的催化剂右键祭坛。环阵可旋转，须保持材料的顺时针顺序。仪式期间底座被锁定，默认每八 tick 运行一步，进度可随世界存档恢复。

用 `/sf storage restore-chunk <dimension> <pos>` 从 `.sfb` 恢复古代祭坛和底座标记。Paper 把底座悬浮物保存在世界实体中，普通 `.sfb` 只有构件 ID。底座旁仍有 Paper 展示实体或记录含无法解析的 `item` 字段时，恢复会保留原档和世界实体并报告位置。可在源 Paper 世界先从底座取出物品，再重新迁移世界与归档。单独归档到正确 `ANCIENT_PEDESTAL` 预设的库存只接受原槽 `0` 的一件规范化物品。

`/sf storage restore-network minecraft:overworld -1 64 -17` 激活已导入的网络节点。也可以在增强工作台制作网络节点并直接放置。能源调节器和连接器可将露天太阳能发电机、电容和电力机器相连。电容电量随方块实体保存，`inspect` 同时显示 Paper 档案电量和当前电量。

增强型自动合成器和合成马达可在增强工作台制作。把机器放在箱子、陷阱箱、木桶或潜影盒上方并接电。在主手拿着已解锁的增强工作台产物，潜行右键机器并确认配方。普通右键查看配方，从底部按钮暂停、恢复或删除选择。机器从同一个容器扣料、放回产物和余下的空瓶或桶，每次成功消耗 16 J，最多储存 256 J。空间不足或区域保护拦截时保留材料和电量。用 `restore-network` 或 `restore-chunk` 激活旧 Paper 头颅，可同时恢复其已选配方与暂停状态。重新导入会保留当前选择。

原版自动合成器使用原版工作台的有序和无序配方，包含数据包中同类的配方。手持产物潜行右键后可用底部翻页玻璃板切换同一产物的不同配方。世界启用 `minecraft:limited_crafting` 时，选择菜单只显示当前玩家已解锁的配方。旧 Paper 头颅中的命名空间配方 ID 可用同一存档命令恢复。

将铁砧放在朝上的发射器上方。解锁 `armor_forge` 和对应装备研究后，把材料按原配方放入发射器，右键铁砧。装备在三秒后进入输出箱或发射器。锻台有 Paper 的 56 条装备配方，锁链装备保留原版物品。已解锁的萤石盔甲、夜视镜、防护服、黏液及蜜蜂装备中有被动药水效果的部件按 Paper 默认周期生效，彩虹盔甲按原色序变色。

穿着蜂翅开始滑翔后，向下飞至露天表面以上四格内会触发三秒缓降。在屋檐下，蜂翅检查脚下六格内是否有阻挡移动的方块。解锁对应研究后，农夫鞋阻止踩坏农田，黏液靴、加固黏液靴和蜜蜂靴阻止普通坠落伤害并播放原落地音效，末影靴保留末影珍珠传送并阻止此次伤害。解锁滑翔帽后，装备它可抵消滑翔撞墙或滑翔结束时的落地伤害，并按 Paper 的耐久和耐久附魔规则磨损帽子，音效只给佩戴者。践踏者靴的反击和防护服的蜂蜇保护仍待补齐，装备交互需要真实客户端验证。

携带放射性物品会按 Paper 原物品的辐射等级和实际数量增加暴露，最高 100。穿齐深潜头盔、防护服胸甲、护腿和橡胶靴，或处于创造与旁观模式时，不再增加暴露或施加新症状，暴露每轮下降一级。原 Paper 玩家存档里的 `slimefun:radiation` 会被读取一次，随后由 Horizon 玩家存档保存。死亡将暴露清零，默认保护 15 秒，该保护期按实际经过的时间计算。可在 `[radiation]` 设置 `enabled`、`interval-seconds` 和 `grace-seconds`，默认每 20 游戏刻计算一次。

装甲自动合成器与手动装甲锻台共享配方，每次成功耗电 32 J，最多储存 256 J。可用带原 Slimefun ID 的锻台产物在菜单选择配方，或从已导入的 Paper 头颅保留选方与暂停状态。普通锁链甲是原版物品，可从旧头颅的 `CHAIN_*` 配方 ID 恢复自动合成。

Paper 的 `stored-chunks/chunks.sfc` 会随快照导出。生成导入档案后，操作员可在对应维度的已加载区块执行 `/sf storage restore-geo minecraft:overworld -1 64 -17`，恢复原油、下界冰、小块铀和盐的已扫描储量。旧档案和 `/sf storage restore-oil` 仍可用于单独恢复原油。耗尽的储量保持为零，重复恢复同一档案会显示当前剩余量。

工业采矿机以高炉为底、箱子为上层，在左右两侧各放一个朝上的活塞，活塞下方使用铁块。高级型把铁块换成钻石块。用 `/sf research industrial_miner` 或 `/sf research advanced_industrial_miner` 解锁，在箱中放入燃料后右击高炉。普通型使用煤炭等固体燃料，搜索下方 7×7 范围并产出原矿等材料。高级型使用岩浆桶、原油桶或燃料桶，在 11×11 范围原样采出矿石，并退回空桶。采矿时按启动者的 UUID 检查每个位置的破坏权限。`[industrial-miner]` 和 `[advanced-industrial-miner]` 可分别设置 `can-mine-deepslate-ores` 和 `can-mine-ancient-debris`。

自动繁殖机和动物生长加速器均提供七个饲料槽，货运可向这些槽送料。放入食品加工机产出的有机食物并接通电源。繁殖机每次消耗一份食物和 60 J，使附近符合繁殖条件的成年动物进入求爱状态。生长加速器每次消耗一份食物和 14 J，让附近的幼体最多提前 2000 游戏刻成长。可用 `/sf storage restore-inventory` 恢复这两种设备的 Paper 饲料槽。

两档作物生长加速器接通电源后使用有机肥料，按 7×7 或 9×9 范围搜索同一高度的作物。每次消耗一份肥料及 25 J 或 30 J，让第一个可施肥的作物生长一阶段。两种设备均支持七槽货运进料及 Paper 库存恢复。普通和农夫可编程仿生机器人可作为这些配方的合成前置。将 Android 放到地面后，用其内存核心编辑脚本，插入移动、转向、等待或接口指令，向燃料槽放入固体燃料后启动。农夫型可收取正前方或下方的成熟作物，并把该作物的年龄重置为零，在原地继续种植。已有 Paper Android 的所有者、燃料、方向、运行位置、脚本和库存可按旧设备记录恢复。未实现运行的 Android 类型保留旧脚本与库存供后续迁移。

三档普通及渔夫 Android 可用原配方制作和安装。渔夫用 `CATCH_FISH` 检查脚下的水，每次尝试的钓获率依档为 10%、20% 和 30%，鱼类来自当前数据包的 `minecraft:fishes` 标签，杂物和宝藏按 Paper 权重抽取。二档消耗岩浆桶、原油桶或燃料桶，空桶进入六个输出槽中可容纳的位置。三档使用铀、镎及强化铀。旧档中同类设备的脚本、燃料余量、进度和库存沿用现有恢复流程。

矿工 Android 有向前、向上和向下挖掘及挖后移动两类指令，按无附魔钻石镐计算原版战利品。方块变更前会检查设备离线属主的领地权限，识别已登记的 Slimefun 方块并跳过。默认按 Paper 优化保留符合周边方块条件的圆石、石头和玄武岩发生器，管理员可用配置 `[miner-android]` 中的 `reduced-block-updates` 关闭。Bukkit 的方块形成扩展事件由 Paper 插件监听，当前 Horizon 矿工未转接该扩展。

普通、农夫、渔夫和矿工 Android 可从内存核心浏览、上传和下载服务器共享的脚本。下载会替换当前脚本。列表支持翻页、下载计数及每账号单次评分，普通脚本在农夫、渔夫及矿工菜单也可见。若迁移了 Paper，通过 `python3 horizon/tools/import-android-scripts.py <Paper 的 plugins/Slimefun/scripts> <Horizon 世界>/horizon-mod-data/slimefun/android-scripts-import.toml` 导入原脚本名称、作者、下载量和评分。导入脚本会出现在对应设备的列表里。首次改动共享库时，Horizon 将导入内容及改动保存至 `android-scripts.toml`。转换脚本使用 PyYAML。

在增强工作台制作并放置 GPS 发射器并接入能源网络，为放置者增加 GPS 复杂度。达到 600 后，可以右击目标区块内的固定 GEO 扫描器，或手持便携扫描器右击目标区块中的方块。扫描会显示按维度和生物群系生成的四种有限储量。接通同一区块的 GEO 矿机即可开采小块铀、下界冰和盐，货运可从矿机的输出槽提取。接通油泵并放入空桶即可提取原油，油桶可送往炼油机，燃料桶可用于内燃发电机。

GPS 标记工具会记录右击方块对应的相邻位置，随后使用玩家下一条聊天消息作为名称。GPS 控制面板可查看自己的在线发射器、复杂度并删除航点。迁移 Paper 航点时运行 `horizon/tools/import-waypoints.py <Paper数据目录> <Slimefun世界数据目录>/waypoints-import --world <Paper世界名>=<Horizon维度>`。使用 PM-Sync 的快照可加 `--server-id`，并用 Paper 世界 UUID 提供对应的映射。玩家须以同一 UUID 加入，导入保留已有同名航点，之后删除的航点不会从同一档案重新加入。

将传送矩阵放在八个 GPS 支柱中间，把个人或共享激活板放在矩阵上即可打开传送菜单。共享激活板会显示矩阵放置者的航点，个人激活板还要求玩家本人是激活板的放置者。便携传送器使用自己的航点，默认在打开菜单时扣除 10 J，并可在配置项 `[portable-teleporter]` 的 `teleportation-cost` 中调整。选择航点后须留在传送源附近，Horizon 会在实际传送时检查出发地与目的地的领地权限。传送矩阵和个人激活板的旧所有者可随 Paper 方块记录恢复。

Auto Enchanter 和 Auto Disenchanter 各有 I、II 两档。分别消耗附魔书或普通书，保留工具的耐久和其他组件。附魔机应用该物品支持的更高等级附魔，去附魔机将直接附魔和铁砧惩罚转移到新书。两个档位的每个处理刻分别消耗 9 J 和 24 J，使用 Paper 原始的输入槽 19、20 和输出槽 24、25，支持货运和库存恢复。前置材料 Wither-Proof Obsidian 可在增强工作台制作，放置后抵抗爆炸和凋灵拆除。当前使用 Paper 的默认处理规则。Bukkit 附魔事件和可选的词条、等级及 Lore 限制仍待移植。

Auto Anvil 和 Auto Anvil Mk.II 用一份 Duct Tape 修理一件有损耗的物品，分别恢复该物品原型最大耐久的 10% 和 25%，向下取整。修理保留附魔、铁砧惩罚和其他物品组件，两档均用 60 个处理刻，沿用 Paper 四个输入输出槽。增强工作台每次生产两份 Duct Tape，机器通过 Slimefun ID 识别胶带。

Book Binder 以书架为载体，将两本附魔书合成一本。相同附魔等阶时提升一级，等级与冲突检查读取服务器当前的附魔注册表。默认沿用 Paper 上限。可在 `slimefun-server.toml` 的 `[book-binder]` 表调整 `bypass-vanilla-max-level`、`has-custom-max-level` 与 `custom-max-level`。

Enhanced Furnace I–XI、Reinforced Furnace 和 Carbonado Edged Furnace 使用原版熔炉界面和库存。各档沿用 Paper 的燃料倍率、加速间隔与指定矿石的幸运产量。普通熔炉、高炉和烟熏炉不受这些规则影响。迁移时用 `/sf storage restore-network` 恢复世界中原有熔炉的 Slimefun 标记，燃料、库存和烹饪状态保留在原版存档中。

将货运输入或输出节点挂到库存方块侧面，再右键配置频道。输入和高级输出节点可放置白名单或黑名单物品。输入节点另可启用轮询和智能填充。空白名单不转移物品。货运支持普通库存和已移植的电动机器，每次按实际源、目标库存位置检查 Horizon 的库存转移许可。目标只能接收部分物品时，源端保留余量。

使用 `/sf research block_placer` 解锁方块放置器，合成后放置发射器并从侧面输入红石信号。它使用实际库存中选中的物品，两 tick 后在朝向的空格放置。库存打开时暂停开始新操作。发射器下方有漏斗以及原版潜影盒使用普通发射行为。

Paper 存档可用 `/sf storage restore-network` 或分块恢复命令恢复方块放置器及其属主。属主离线时仍按 UUID 检查建造许可。早期未保存属主的记录保留 Paper 的行为。可在 `[block-placer]` 的 `unplaceable-blocks` 配置额外的材料名单，覆盖默认不可破坏材料名单。
