# Slimefun for Horizon 26.3

本分支从 PigeonMoe Slimefun 5.0.1 的 `26.2` 分支（`f6d16b6`）移植，保留原有 GPLv3 许可证。已发布版本为服务端 WebAssembly 预览，提供部分多方块机器。全量 1:1 移植仍在开发。

## 安装

1. 从同一预发布下载 `slimefun.hmod` 和 `horizon-wasm-std.wasm`，使用 `SHA256SUMS` 核对文件。使用该预发布标明的 PigeonMoe Horizon Fork 提交构建服务端，将配套的 `horizon-wasm-std.wasm` 放在游戏引擎可执行文件旁。
2. 将文件放进服务端游戏目录的 `horizon-mods/`，在该游戏目录的 `horizon-mods.txt` 添加 `mod.slimefun:true`，然后重启。
3. 如使用 PigeonMoe Horizon Panel，将 [slimefun.toml](panel-plugins/slimefun.toml) 放进服务端实例目录的 `panel-plugins/`。启用面板后，该条目调用固定的 `slimefun status` 查询命令。

## 游戏内

- `/sf guide` 发放生存指南，`/sf open_guide` 打开指南，`/sf recipe <物品>` 显示配方，`/sf status` 显示覆盖范围。
- 将橡木栅栏放在朝上的发射器上方，装入素材后对栅栏点右键。磨石包含原分支的 26 条默认配方，包括烈焰棒、下界疣、末影之眼和小麦。
- 将工作台放在发射器上方，在发射器内排列 3×3 配方后对工作台点右键。增强工作台包含四条魔法块及末影块升级配方和一条海带饼干配方。
- `/sf research lumps` 和 `/sf research kelp_cookie` 分别花费 3 级和 4 级经验。创造模式可免费使用。研究解锁保存在 Horizon 玩家的持久数据中。
- 在线操作员可运行 `/sf charge` 将主手的可充电远古工艺物品充满。操作员和受信服务端命令源可运行 `/sf give <玩家名> <物品 ID> [数量]` 给在线玩家发放物品。发放会优先合并同组件的未满堆，再使用空槽。`[options]` 下的 `drop-excess-sf-give-items` 默认关闭，开启后会在玩家处掉落放不进背包的部分。
- `/sf stats` 查看自己的研究称号、进度和已解锁研究的经验等级合计。`/sf stats <玩家名>` 供在线操作员和控制台查询指定在线玩家。称号沿用 Paper 默认顺序，可用配置文件顶层的 `research-ranks` 数组覆盖。
- 操作员可用 `/sf versions` 查看当前 Horizon、Minecraft、远古工艺和已加载附属插件的版本。附属插件通过其声明的必需或可选依赖识别，反馈链接来自各自的清单。
- 在线操作员可用 `/sf teleporter [玩家名]` 打开自己或指定账号的航点菜单，从当前位置传送。指定账号可以在线，也可以是专用服务端用户缓存中仍有有效记录的离线玩家。命令沿用 Paper 的管理传送速度，无需便携传送器或 GPS 矩阵。
- 操作员可用 `/sf timings [--avg|--low]` 查看最近一轮完成的网络设备、强化熔炉和彩虹方块换色耗时，按物品、区块和插件查看。默认按总耗时降序，`--avg` 将物品分组按每台设备的平均耗时降序排列，`--low` 按总耗时升序。控制台可加 `--verbose` 显示所有条目。报告分别采样发电机与货运输入节点，管理器耗时扣除这些子操作。已加载的强化熔炉按 `[URID]` 的 `custom-ticker-delay` 在原版熔炉更新前采样，空闲的强化熔炉也计入。

磨石与增强工作台优先使用已标记的输出箱，其次使用发射器。受保护的发射器会检查领地权限，产物空间不足时回滚库存变更。支持的自定义物品使用原来的 `slimefun:slimefun_item` 持久键和原始 ID。

## 覆盖范围

| 已实现 | 尚待移植或验证 |
| --- | --- |
| 磨石、增强工作台、输出箱、生存矿物处理链及电动机器。指南收录原版简中目录、机器加工页与原版配方链接 | 按玩家切换指南语言及尚未核对的物品与机器边界行为 |
| 太阳能、燃料发电、反应堆、电量缓冲、货运节点与过滤 | 网络配置细节及各路线的真实客户端验收 |
| 研究、机器菜单、GEO 储量、共享库存以及普通和特殊背包的 Paper 存档恢复。Horizon 可接收通过 MySQL 权威验证的 Paper 已释放档案 | 其他 Paper 数据类型及持续双向同步 |
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

## 生存指南

`/sf guide` 发放带原 Paper 生存指南标记的「远古工艺之书」。右击这本书或执行 `/sf open_guide`，可在简体中文指南中浏览已移植的分类。点击锁定物品的屏障可花费经验等级解锁研究。配方页按原九槽布局显示材料、制作方法和成品。需要基础机器研究的分类在其已移植的前置研究完成后开放。

点击指南右上角的命名牌后在聊天输入关键词，或执行 `/sf search <关键词>`。搜索按名称和 Paper 物品 ID 匹配，显示前 35 项。关闭后再次打开指南会回到上次浏览的位置。配方页和搜索结果可以返回前一页，潜行点击返回按钮可回到主页。

指南按 Paper 配方布局显示切割机、自动淘金机及两档工业采矿机的摆法，还可查到有机食品、有机肥料、洗矿机矿粉、淘金所得的筛矿与铁傀儡掉落的基础电路板。

新玩家首次进入世界时会自动获得指南。可在 `[guide]` 下设置 `receive-on-first-join = false` 关闭赠送。

潜行右击指南可查看 PigeonMoe 移植信息和源码，还可启用或关闭解锁动画。二级及以上管理员可在此进入作弊模式，点击领取物品，潜行点击领取一组。多方块结构仍需手动搭建。

Horizon 会读取 Paper 玩家档案保存的学习动画选项。开启时，点击锁定物品只扣一次经验等级，在屏幕中央显示研究名称和进度条。第 100 刻解锁，短暂显示完成后返回指南。退出或重启后从最后保存的进度继续。

二级及以上管理员和控制台可使用 `/sf research all <玩家名>` 解锁指定在线玩家的全部研究，或用 `/sf research clear <玩家名>` 清除其研究和正在进行的解锁。两条命令都不会改变经验等级。

`[researches]` 的 `free-in-creative-mode` 默认为 `true`，创造模式玩家可立即免费学习。`disable-learning-animation = true` 会为整个服务器关闭学习动画。解锁研究时默认发射 Paper 研究烟花。玩家可以在指南设置里关闭烟花，服主可以设置 `enable-fireworks = false`。旧 Paper 玩家档案里的烟花设置仍然可用。研究烟花保留爆裂效果并免除爆裂伤害。

知识共享典籍在魔法工作台制作，需先解锁 `tome_of_knowledge_sharing`（30 级）。第一次右键按玩家 UUID 绑定，转交后右键可复制持有人已完成的研究。在线读取实时研究，离线读取该 UUID 的原版玩家档案和已导入的 Paper 研究。复制无需经验，生存模式消耗一册，创造模式保留。

已有物品的节日分类按服务器本地月份显示。`[guide]` 的 `show-hidden-item-groups-in-search` 默认为 `false`。启用后，搜索可找到当月隐藏的节日物品。指南支持按中文名称、英文名称和原物品 ID 搜索。

复活节彩蛋在增强工作台一次制作两枚，右键时在玩家脚边随机掉落一份礼物并发射两枚烟花。圣诞礼物在魔法工作台制作，右键方块时在点击面外侧掉落一份礼物并发射三枚烟花。创造模式保留手中的礼物，两种物品全年可制作。

## 护符

魔法工作台可制作普通护符、魔法玻璃以及 Magician、Wizard、Wise、Firefighter、Angel 和 Traveller 护符。魔法玻璃用于配方，不能直接放置。六种护符均可用四枚 Ender Lump III 进行末影灌注。普通护符放在主物品栏，灌注版放在末影箱，两者均需完成相应的基础护符研究。生存指南的二阶护符分类需先完成已移植的一阶护符研究。

Magician 可从当前注册表中选择相容附魔。默认仅对可接受该附魔的装备生效。服务器可在 `[magician-talisman]` 启用 `allow-enchantment-books`，或在 `[magician-talisman.allow-enchantments.minecraft.efficiency.level]` 将 `"2"` 设为 `false`，停用该附魔等级。Wizard 可在有资格的工具上给出三级至五级时运，精准采集出现在当次附魔列表时不会触发。Wise 只尝试翻倍经验球修补后剩余的经验。使用 `[talismans]` 的 `use-actionbar = false` 可改为聊天提示。

Firefighter 原版与末影灌注版每次制作四枚。完成 `fire_talisman` 研究后，玩家直接接触火焰或灵魂火焰时会先使用主物品栏的 Firefighter，再查找末影箱的灌注版。每次成功触发消耗一枚，免除该次接触伤害并给予 180 秒抗火 V。已免疫的伤害和原版受伤冷却拦下的伤害不会消耗护符。服上有插件拒绝抗火效果时，保留护符并照常结算伤害。触发条件限于直接接触火焰。

Angel 在鞋靴、缓降和受伤冷却已处理后，按 Paper 的 0–75 随机边界尝试免除坠落伤害。Traveller 在玩家开始冲刺时按 0–60 随机边界尝试给予 180 秒速度 III，已有任意等级速度效果时保留原效果。两种护符原版和末影灌注版每次制作一枚，触发时保留在原槽。

## 附魔符文

用古代祭坛制作闪电符文，再用四块魔法玻璃制作附魔符文。研究 Ancient Enchanting 后，将附魔符文和一件装备投到一起。符文等待 20 刻后查找附近第一件非附魔符文的掉落物。从支持该装备且与已有附魔相容的选项中随机抽选，再显示无伤害的闪电。10 刻后仍能验证两件原掉落物时，才消耗符文并生成保留原组件的附魔成品。

绑定诅咒和消失诅咒不会被选中。服主可用 `[items.<Paper ID>]` 下的 `allow-enchanting = false` 阻止符文给对应 Slimefun 物品附魔。

## 抓钩

研究 `grappling_hook` 后，可用增强工作台制作钩头、链和抓钩。朝空中右击会发射实体箭矢，箭矢在普通方块上扎住后牵引玩家。生存模式默认先消耗一个抓钩，成功命中后在落点留下可捡回的抓钩。副手持弓时不能发射。`[grappling-hook]` 的 `consume-on-use` 默认开启，`despawn-seconds` 保留 Paper 配置名，数值按游戏刻计算，默认为 60。此功能需要支持箭矢作为牵绳端点的 Horizon 底座。

立即牵引以普通生物或玩家的准许伤害、画或物品展示框的实际破坏，以及可用传送门的接触为准。远距离牵引先请求上移半格。领地否决这次移动时结束牵引，落点仍会留下可拾回的抓钩。

## 多功能工具

前六档多功能工具使用 `multitools` 研究，Carbonado 使用 `carbonado_tools`。潜行右键依次切换模式，模式保存在同一个 Paper 物品键 `slimefun:multitool_mode` 中。普通右键消耗 0.3 J，通过充电台补充。前六档有便携工作台、魔法末影之眼、风杖、抓钩四种模式，Carbonado 还可在对应方块上使用普通和下界淘金盘。

用多功能工具发射抓钩时，剪刀留在手里，命中后直接牵引。风杖模式要求至少两点饥饿值。工具的淘金模式沿用领地挖掘权限。

服主可在例如 `[multi-tool.DURALUMIN_MULTI_TOOL.mode.1]` 下设置 `enabled = false`，潜行切换时会跳过这项。用 `item` 可指定已移植的模式物品 ID。

## 本地开发增量

本节描述开发分支的变更。使用当前 PigeonMoe Horizon Fork 构建以取得最新的设备和背包接口。

在 `[solar-generator-4]` 中启用 `other-dimensions-use-night-energy` 后，第四档太阳能发电机可在下界和末地每次网络更新提供 64 J。默认关闭。使用原版主世界维度类型的额外世界也按各自的时间、天气和天空光照发电。

手持淘金盘、自动淘金机和三档电动淘金机共用 Paper 掉落权重。可在 `[items.GOLD_PAN.chance]` 配置 `FLINT`、`CLAY`、`SIFTED_ORE`、`IRON_NUGGET`，在 `[items.NETHER_GOLD_PAN.chance]` 配置 `QUARTZ`、`GOLD_NUGGET`、`NETHER_WART`、`BLAZE_POWDER`、`GLOWSTONE_DUST`、`GHAST_TEAR`。正数按权重合计归一，0 会停用该掉落物。对应盘的权重全为 0 时，电动版本保留原料和电量，手持及自动版本照常处理原料，产物为空。

电炉和电动矿物粉碎机各有三档，电动锭粉碎机有一档，加热压力舱有两档。这些机器通过能源调节器取电，保存输入、输出、剩余进度及缓冲电量。当前分支已覆盖 Paper 底版的 77 个电力分类物品 ID。具体配方、配置和边界行为继续按 Paper 核对。

研究 `multimeter` 后，可用万用表右击有储能容量的设备查看当前电量和容量。太阳能头盔使用 `solar_panel_and_helmet` 研究，默认在指定日照时间且眼部天空光为 15 时，每 10 刻给手持及穿戴的可充电物品各充 0.1 J。可在 `[solar-helmet]` 调整 `enable-armor-effects`、`armor-update-interval` 和 `charge-amount`。

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

四项设置保存在该 Horizon 存档的 `slimefun-server.toml`。默认值为 `double-ores = true`、`legacy-ore-washer = false`、`fire-breaking-chance = 34` 和 `output-chest-enabled = true`。如需在下界禁用输出箱，在文件末尾加入 `[worlds."minecraft:the_nether"]` 和 `output-chest-enabled = false`。禁用后输出箱从该维度的指南中隐藏，玩家和方块放置器无法放置新的输出箱，机器会跳过已放置的输出箱。恢复 Paper 存档时仍保留原箱与标记。

常规冶炼炉的发射器下方可安装点火室，用其中的打火石维持火焰。获准的 TNT 爆炸和启用了爆炸事件的爆炸工具会销毁带标记的输出箱及其内容。领地拦截爆炸时会保留箱体与库存。

### 下界淘金盘

本地开发版 `0.1.0-dev.4` 加入下界淘金盘。花费 8 级经验研究 `nether_gold_pan`，在增强工作台用普通淘金盘和五块下界砖制作。右击灵魂沙或灵魂土时会检查方块破坏权限，再按 Paper 默认权重从石英、金粒、下界疣、烈焰粉、荧石粉和恶魂之泪中产出一件物品。可用 `/sf recipe ghast_tear` 等命令查询权重。

当前开发版可通过本节上方的 `[items.NETHER_GOLD_PAN.chance]` 设置自定义掉落权重。Bukkit 物品生成事件仍待移植。

### Paper 方块存档与基础网络

使用 PyYAML 将 Paper 的 Slimefun 方块元数据和容器档案转换到已迁移的 Horizon 世界。先停止 Paper 服务器，或对同一时间点的快照执行导入。

```sh
python3 horizon/tools/import-block-storage.py \
  /path/to/paper/data-storage/Slimefun \
  /path/to/horizon-world/horizon-mod-data/slimefun/storage-import
```

工具默认识别 `world`、`world_nether` 和 `world_the_end`。使用 `--map-world survival=minecraft:overworld` 指定其他世界名。也可以直接传入 Paper `block-backups` 生成的 ZIP 作为输入。原始 ZIP、`.sfb` 和 `.sfi` 保持原样。导入目录已有不同数据时，工具会保留该目录并报错。使用旧版工具生成的目录需要先备份，再用新版工具重新生成。

二级及以上 OP 可用 `/sf storage` 查看统计，再使用 `/sf storage inspect minecraft:overworld -1 64 -17` 查看具体位置。对于已迁移且仍是普通箱子的输出箱，使用 `/sf storage restore-output minecraft:overworld -1 64 -17` 恢复路由标记，原有箱内物品保持原样。目标需位于当前维度的已加载区块。

`/sf storage restore-inventory minecraft:overworld -1 64 -17` 可恢复已移植的电力机器、货运输入节点和高级货运输出节点的菜单库存。电炉系列和高温压力舱都使用 Paper 的原始输入槽 19、20 与输出槽 24、25。反应堆与反应堆访问端口在原来的 19、28、37 槽恢复燃料，25、34、43 槽恢复冷却剂，40 槽恢复副产物。菜单会根据恢复后的运行模式和连接重新生成按钮。旧 `.sfi` 缺少 `preset` 时，已移植的 Slimefun 方块菜单可按同一坐标的 `.sfb` 方块 ID 恢复。若两份档案明确给出不同 ID，恢复会报错并保留原档。命令保留物品数量，使用每件物品的 DataVersion 升级组件。任何一个槽位无效时整份菜单均不写入。带旧式 `type` 或 `meta` 字段的物品保留在原始档案中，`inspect` 会标明需先由 Paper 重新保存的槽位。

`/sf storage restore-chunk minecraft:overworld -1 64 -17` 按位置顺序恢复指定区块已支持的方块。每批最多 32 条，达到当前命令的处理时间后会在完整记录之间暂停。命令会列出拒绝恢复的坐标和原因，也会给出继续使用的偏移量。未移植的方块仍保存在原档案中。

若仅有 `.sfi`，`restore-inventory` 和 `restore-chunk` 可为同一位置已注册且保存标记一致的受支持 Slimefun 方块恢复原槽。旧档缺少 `preset` 时沿用 Paper 的规则，使用该方块的注册 ID。档案明确给出预设时，预设仍须与方块一致。方块现有的电量、属主和设置保留。恢复成功后，同一位置的区块记录会阻止再次补入物品，包括拆除后放置同类型机器的情况。

垃圾桶可用 `restore-inventory` 或 `restore-chunk` 恢复 Paper 的 10–16 号进料槽。恢复的物品会随垃圾桶下一次运行销毁，重复恢复同一份归档不会补回已销毁的物品。

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

使用支持版本化 gzip 物品 NBT 的 Horizon SDK 时，可以把导出的 `SFP1` 目录内容直接放进世界内的 `horizon-mod-data/slimefun/pm-sync-import/`。原玩家进入时，插件会合并已知的研究键和历史数字 ID，并保留档案中的首次发书状态。用 `/sf restore-backpack` 或 `/sf backpack` 恢复背包时，Horizon 会按原编号、原容量和原槽位读取该档案，并用对应的 DataVersion 转换物品。已有的标准 TOML 背包档案优先，已恢复的当前背包不会被旧档覆盖。未识别的附属插件研究仍保存在原始 SFP1 文件中。

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

魔法工作台以两份来世精华把原版武器、工具或防具制成灵魂绑定版。对应研究是 `bound_weapons`、`bound_tools` 或 `bound_armor`。`special_elytras` 在古代祭坛解锁自带经验修补的灌魔鞘翅和灵魂绑定鞘翅。带原 Paper 物品身份的这些装备死亡后仍保留在原槽。

解锁 `seismic_axe` 后，在魔法工作台用三块硬化金属锭和两根元素法杖制作地震斧。右键沿视线掀起地面方块的影像，对附近可攻击的生物施加一次击退和伤害。影像落地即消失，兼容原版 PvP 和领地伤害规则。

研究 `soulbound_rune` 后可在古代祭坛制作灵魂绑定符文。把符文丢到一件已可拾取的物品附近，20 刻后会在距符文 1.5 格的范围内选择第一件兼容物品并显示闪电。再过 10 刻，若两者仍在，仪式消耗整堆丢出的符文并重新掉落绑定物品。被绑定的物品死亡时保留在原槽。旧 Paper 的绑定标记也适用于普通物品。

已有的 Paper 灵魂绑定背包可用同一命令恢复 36 格库存。灵魂绑定背包和带既有 Soulbound 标记的其他背包会在死亡时留在原槽，保留物品规则和消失诅咒继续由原版处理。右键末影背包会打开当前玩家的原版末影箱，与实际末影箱共用同一份库存。两款背包都在魔法工作台制作。用编织背包合成灵魂绑定背包时，原所有者 UUID、编号、36 格内容和已有 Soulbound 标记随成品保留。Horizon 的 SFP1 转服数据包括背包、研究和首次发书状态。启用 MySQL 权威验证后，Horizon 可接收 Paper 已释放的档案。

在魔法工作台制作古代祭坛和底座。在祭坛同一高度，四个底座各离中心三格，其余四个分别沿对角线在 X、Z 各偏移两格。用 `/sf recipe ESSENCE_OF_AFTERLIFE` 或对应符文 ID 查询配方。把环上八件材料逐件右键放到底座上，手持配方中心的催化剂右键祭坛。环阵可旋转，须保持材料的顺时针顺序。仪式期间底座被锁定，默认每八 tick 运行一步，进度可随世界存档恢复。

在魔法工作台以硅铁和镀金铁制作收容镐。解锁 `pickaxe_of_containment` 后，采集已配置生物的原版刷怪笼会获得保留物种的破损刷怪笼，此次采集不掉经验。解锁 `repaired_spawner` 后，可在古代祭坛用两个末影符文、两份来世精华和四个装满的经验瓶完成修复。修复后的刷怪笼可实际放置，也能由方块放置器安装；再次以收容镐采集会保留修复状态和当前物种。默认可用刷怪蛋改种，`[reinforced-spawner]` 的 `allow-spawn-eggs` 可关闭此功能。

若已迁移 Paper 世界中的强化刷怪笼和对应方块档案，可用 `/sf storage restore-network minecraft:overworld <x> <y> <z>` 为原位置恢复强化身份，也可用 `restore-chunk` 批量恢复。原版刷怪笼实体的当前物种、刷怪权重、延时与数量都保留。

用 `/sf storage restore-chunk <dimension> <pos>` 从 `.sfb` 恢复古代祭坛和底座标记。普通 `.sfb` 只有构件 ID，Paper 的底座物品保存在世界实体中。若在原位置找到唯一匹配的探针物品与其盔甲架，恢复会移除探针名称，保留原自定义名称和物品组件，把物品写回底座，再移除旧展示实体。带 Paper 原探针格式的展示物在待迁移时不会自然消失。选择含 `ANCIENT_PEDESTAL` 预设库存的单独归档时，只接受原槽 `0` 的一件规范化物品。若两种来源同时存在、实体配对不完整或 `item` 元数据无法解析，恢复会保留原档和世界实体并报告位置。

彩虹古代符文以末影符文为祭坛中心，每次获得一枚。六种彩虹方块可在古代祭坛制作，普通版每次产出八个，圣诞、情人节和万圣节版每次产出两个。圣诞配方使用增强工作台制作的圣诞饼干，每次获得十六块。配方全年可用。普通版依次循环 16 色，节日版分别使用红／绿、洋红／粉和橙／黑。同一变体在已加载区块内按 `[URID]` 的 `custom-ticker-delay` 同步换色，Paper 默认为每 10 游戏刻。染色玻璃板保留四边连接与水涝，玩家破坏后掉落对应的 Slimefun 彩虹方块，爆炸会摧毁方块并且没有掉落。现有 Paper 方块档案可用 `/sf storage restore-network` 或 `restore-chunk` 恢复标记。Horizon 的 `/sf recipe` 全年显示这些配方。Paper 的节日月份仅影响原指南分类的可见性。

`/sf storage restore-network minecraft:overworld -1 64 -17` 激活已导入的网络节点。也可以在增强工作台制作网络节点并直接放置。能源调节器和连接器可将露天太阳能发电机、电容和电力机器相连。电容电量随方块实体保存，`inspect` 同时显示 Paper 档案电量和当前电量。

货运迁移保留 Paper 所有者 UUID、频道和过滤选项。对已激活的同一节点补导 `.sfi` 时，仅在过滤槽仍空且从未恢复时填入归档物品，保留当前频道和开关。恢复后取走的过滤物品不会在重跑时补回。现有网络标记损坏时停止恢复并保留原数据。

货运过滤区分普通、延长和增强等药水基型，包括药箭。开启比较物品说明时，仍检查两件物品是否都有说明，再比较灰色灵魂绑定行以外的内容。同一档 Slimefun 背包按其物品 ID 匹配，所有者和背包编号由背包库存单独保存。

`[networks]` 的 `max-size` 默认为 200，控制每轮发现扫描的位置数。空位也占用扫描额度，尚未扫完的位置留到下一轮，已连接的节点继续工作。电力、货运及同轮运行的设备默认每 10 游戏刻更新，可在 `[URID]` 的 `custom-ticker-delay` 调整。货运的 `cargo-ticker-delay` 按这个设备周期计数。

货运余量优先退回原槽。原槽被占用时，原版容器先合并物品和组件均一致的堆叠，再使用空槽。最终无法退回的余量默认在输入容器上方掉落。将 `[networks]` 的 `delete-excess-items` 设为 `true` 可销毁这部分余量。

增强型自动合成器和合成马达可在增强工作台制作。把机器放在箱子、陷阱箱、木桶或潜影盒上方并接电。在主手拿着已解锁的增强工作台产物，潜行右键机器并确认配方。普通右键查看配方，从底部按钮暂停、恢复或删除选择。自动合成器启动前需要同一容器至少有一个空位，扣除材料后先放入产物，再将能放下的空瓶或桶放回容器。放不下的空瓶或桶会被丢弃。产物放不下或区域保护拦截时保留材料和电量。增强型每次成功消耗 16 J，最多储存 256 J。用 `restore-network` 或 `restore-chunk` 激活旧 Paper 头颅，可同时恢复其已选配方与暂停状态。重新导入会保留当前选择。

原版自动合成器使用原版工作台的有序和无序配方，包含数据包中同类的配方。手持产物潜行右键后可用底部翻页玻璃板切换同一产物的不同配方。机器从服务端配方表选取配方。启用 `minecraft:limited_crafting` 时，玩家的配方书和手动合成仍遵循原版规则。旧 Paper 头颅中的命名空间配方 ID 可用同一存档命令恢复。

将铁砧放在朝上的发射器上方。解锁 `armor_forge` 和对应装备研究后，把材料按原配方放入发射器，右键铁砧。装备在三秒后进入输出箱或发射器。锻台有 Paper 的 56 条装备配方，锁链装备保留原版物品。已解锁的萤石盔甲、夜视镜、防护服、黏液及蜜蜂装备中有被动药水效果的部件按 Paper 默认周期生效，彩虹盔甲按原色序变色。

穿着蜂翅开始滑翔后，向下飞至露天表面以上四格内会触发三秒缓降。在屋檐下，蜂翅检查脚下六格内是否有阻挡移动的方块。解锁对应研究后，农夫鞋阻止踩坏农田，黏液靴、加固黏液靴和蜜蜂靴阻止普通坠落伤害并播放原落地音效，末影靴保留末影珍珠传送并阻止此次伤害。解锁滑翔帽后，装备它可抵消滑翔撞墙或滑翔结束时的落地伤害，并按 Paper 的耐久和耐久附魔规则磨损帽子，音效只给佩戴者。解锁践踏者靴后，普通坠落会使玩家反弹，并击退附近具有重力的存活实体。PvP 和领地权限只限制反击伤害。反击沿用原版无攻击者的伤害来源，装备交互需要真实客户端验证。

携带放射性物品会按 Paper 原物品的辐射等级和实际数量增加暴露，最高 100。穿齐深潜头盔、防护服胸甲、护腿和橡胶靴，或处于创造与旁观模式时，不再增加暴露或施加新症状，暴露每轮下降一级。原 Paper 玩家存档里的 `slimefun:radiation` 会被读取一次，随后由 Horizon 玩家存档保存。死亡将暴露清零，默认保护 15 秒，该保护期按实际经过的时间计算。可在 `[radiation]` 设置 `enabled`、`interval-seconds` 和 `grace-seconds`，默认每 20 游戏刻计算一次。 完整防护服也能抵消蜂蜇的直接伤害，每件按 Paper 耐久规则磨损。获准的蜇击仍会留下蜂针并施加原版毒素。

装甲自动合成器与手动装甲锻台共享配方，每次成功耗电 32 J，最多储存 256 J。可用带原 Slimefun ID 的锻台产物在菜单选择配方，或从已导入的 Paper 头颅保留选方与暂停状态。普通锁链甲是原版物品，可从旧头颅的 `CHAIN_*` 配方 ID 恢复自动合成。

Paper 的 `stored-chunks/chunks.sfc` 会随快照导出。生成导入档案后，操作员可在对应维度的已加载区块执行 `/sf storage restore-geo minecraft:overworld -1 64 -17`，恢复原油、下界冰、小块铀和盐的已扫描储量。旧档案和 `/sf storage restore-oil` 仍可用于单独恢复原油。耗尽的储量保持为零，重复恢复同一档案会显示当前剩余量。

运行 `horizon/tools/import-block-storage.py` 时可加 `--resources-config <Paper 的 resources.yml>`。将导出目录中 `settings-geo.toml` 的 `[geo-resources]` 节合并到服务端 `slimefun-server.toml` 后重启。此节支持 `oil`、`nether_ice`、`uranium` 和 `salt` 四个布尔开关，默认开启。关闭的资源从扫描结果和采矿候选中移除，历史储量继续保留。关闭 `oil` 时抽油泵暂停。

工业采矿机以高炉为底、箱子为上层，在左右两侧各放一个朝上的活塞，活塞下方使用铁块。高级型把铁块换成钻石块。用 `/sf research industrial_miner` 或 `/sf research advanced_industrial_miner` 解锁，在箱中放入燃料后右击高炉。普通型使用煤炭等固体燃料，搜索下方 7×7 范围并产出原矿等材料。高级型使用岩浆桶、原油桶或燃料桶，在 11×11 范围原样采出矿石，并退回空桶。采矿时按启动者的 UUID 检查每个位置的破坏权限。`[industrial-miner]` 和 `[advanced-industrial-miner]` 可分别设置 `can-mine-deepslate-ores` 和 `can-mine-ancient-debris`。

自动繁殖机和动物生长加速器均提供七个饲料槽，货运可向这些槽送料。放入食品加工机产出的有机食物并接通电源。繁殖机每次消耗一份食物和 60 J，使附近符合繁殖条件的成年动物进入求爱状态。生长加速器每次消耗一份食物和 14 J，让附近的幼体最多提前 2000 游戏刻成长。可用 `/sf storage restore-inventory` 恢复这两种设备的 Paper 饲料槽。

两档作物生长加速器接通电源后使用有机肥料，按 7×7 或 9×9 范围搜索同一高度的作物。每次消耗一份肥料及 25 J 或 30 J，让第一个可施肥的作物生长一阶段。两种设备均支持七槽货运进料及 Paper 库存恢复。普通和农夫可编程仿生机器人可作为这些配方的合成前置。将 Android 放到地面后，用其内存核心编辑脚本，插入移动、转向、等待或接口指令，向燃料槽放入固体燃料后启动。农夫型可收取正前方或下方的成熟作物，并把该作物的年龄重置为零，在原地继续种植。已有 Paper Android 的所有者、燃料、方向、运行位置、脚本和库存可按旧设备记录恢复。旧脚本中无法辨识的扩展指令会保留在存档内。

三档普通及渔夫 Android 可用原配方制作和安装。渔夫用 `CATCH_FISH` 检查脚下的水，每次尝试的钓获率依档为 10%、20% 和 30%，鱼类来自当前数据包的 `minecraft:fishes` 标签，杂物和宝藏按 Paper 权重抽取。二档消耗岩浆桶、原油桶或燃料桶，空桶进入六个输出槽中可容纳的位置。三档使用铀、镎及强化铀。旧档中同类设备的脚本、燃料余量、进度和库存沿用现有恢复流程。

矿工 Android 有向前、向上和向下挖掘及挖后移动两类指令，按无附魔钻石镐计算原版战利品。方块变更前会检查设备离线属主的领地权限，识别已登记的 Slimefun 方块并跳过。默认按 Paper 优化保留符合周边方块条件的圆石、石头和玄武岩发生器，管理员可用配置 `[miner-android]` 中的 `reduced-block-updates` 关闭。Bukkit 的方块形成扩展事件由 Paper 插件监听，当前 Horizon 矿工未转接该扩展。

伐木 Android 用 `CHOP_TREE` 沿 Paper 的十个相邻方向按深度优先查找，最多访问 160 根连通原木。设备每步砍一根，完成前继续同一条指令。它按属主的领地权限砍伐，最低层按原树种和土壤补苗，土壤不合适时掉落幼苗。Paper 当前植树表能够识别的树种沿用其默认值，新版本原木若没有已知幼苗则只砍伐。进阶农夫使用液体燃料及两个原进阶指令，能收获并补种原版作物。旧 ExoticGarden 作物需要该附加插件，原脚本指令仍可导入。

三档屠宰 Android 可用四条攻击指令按 Paper 的怪物、动物和成年动物条件筛选设备前方的目标。设备不攻击玩家及盔甲架，燃料与输出槽布局沿用其他 Android。三档检测半径分别为 5、6、7 格，单次伤害分别为 4、8、20。普通生物死亡后一刻收集附近可拾取物品，补充 Paper 的烈焰棒、凋灵骷髅头和卫道士绿宝石产出，并按原版经验球合并规则给予 1–6 点经验。末影龙死亡走引擎的独立流程。

所有原版 Slimefun Android 均可从内存核心浏览、上传和下载服务器共享的脚本。下载会替换当前脚本。列表支持翻页、下载计数及每账号单次评分，普通脚本也会出现在其他 Android 类别的共享库里。若迁移了 Paper，通过 `python3 horizon/tools/import-android-scripts.py <Paper 的 plugins/Slimefun/scripts> <Horizon 世界>/horizon-mod-data/slimefun/android-scripts-import.toml` 导入原脚本名称、作者、下载量和评分。导入脚本会出现在对应设备的列表里。首次改动共享库时，Horizon 将导入内容及改动保存至 `android-scripts.toml`。转换脚本使用 PyYAML。

急救研究解锁布料、罐头和五种医疗用品。布条回复两颗心，绷带回复四颗心，两者也能熄灭玩家身上的火。夹板回复两颗心。维生素和药剂回复四颗心、灭火、解除辐射与八种指定负面效果。用牛奶制作浓奶油会在增强工作台留下空桶。药剂使用浓奶油配制，按住使用键饮用后留下玻璃瓶。

元素法杖及风、水、火三种变体在魔法工作台制作。风杖沿视线推进，需至少两格饥饿。水杖可熄灭玩家身上的火，火杖拥有火焰附加 V 并且无法通过自动去附魔机拆出。服务器配置的 `[wind-staff]` 下可设 `power`，默认 4。

魔法末影之眼在魔法工作台制作。穿齐 Slimefun 末影盔甲后右键可以投掷一颗原版末影珍珠，眼本身会保留，传送遵循服务器的末影珍珠规则。

幸运饼干保留原版曲奇的食用条件，吃完会随机显示一条 Paper 英文签文。减重饼干用鞘翅鳞片在魔法工作台制作，食用后以 3 秒漂浮 II 替换原漂浮效果。魔法糖右键食用，给予 30 秒速度 IV。右击发射器或附魔台时可保留魔法糖作为祭坛材料。怪物肉干用盐和腐肉制作，食用后一刻清除饥饿药水效果，再给予 5 刻饱和 I。

熔炼镐由三块熔岩水晶和两块硅铁在增强工作台制成。研究解锁后，用主手熔炼镐挖掘指定矿石或远古残骸时，掉落会按世界当前的熔炉配方换成烧制产物。时运沿用 Paper 的额外倍率，精准采集仍按熔炉配方处理单件掉落，破坏经验由原版产生。领地拒绝破坏时不会结算这些效果。

矿脉镐和寻矿镐均在魔法工作台制作。矿脉镐顺着相邻及同层斜角逐块采掘矿石，默认最多 16 块。每块单独检查破坏权限，矿脉采掘按 Paper 规则发放掉落，整次不额外消耗耐久或产生经验。用寻矿镐右键会转向 5 格搜索范围内最近的矿石，查无矿时也会按耐久规则结算。两个范围可分别用 `[pickaxe-of-vein-mining]` 的 `max-blocks` 和 `[pickaxe-of-the-seeker]` 的 `max-range` 设置。

爆炸镐和爆炸铲在魔法工作台制作。研究解锁后，站立挖掘会按工具类型检查中心周围的 26 个候选位置，逐块遵守破坏许可。附加方块沿用手持工具的掉落和 Slimefun 标记恢复，按 `damage-on-use` 设置扣耐久，中心方块由常规挖掘结算。潜行时只挖中心方块。打开 `call-explosion-event` 会先让保护监听器筛除爆炸候选，再逐块检查玩家权限。领地拒绝该次附加破坏时，已经获准的中心方块仍由常规挖掘结算。

次元偏转卷轴在魔法工作台制作。右键后，将范围内的其他玩家和生物原地转向 180°，忽略盔甲架，保持卷轴数量、实体坐标和俯仰角。玩家位移检查仍可拒绝转向。半径可在 `[scroll-of-dimensional-teleposition]` 下设置，默认 10 格。

斩首剑在增强工作台制作。解锁研究后，系统以受害者的原版玩家击杀归属和击杀者当前主手判断头颅掉落。六种支持的目标分别使用 `[sword-of-beheading.chance]` 的整数百分比，玩家头颅继承真实 UUID、名称和登录皮肤属性。

吸血之刃和寒冰弓在魔法工作台制作。吸血之刃命中后默认有 45% 概率恢复 4 点生命，可在 `[blade-of-vampires]` 中调整 `chance`。寒冰弓按发射时的弓识别箭矢，命中施加 40 刻的减速和跳跃效果，玩家另获得 60 刻冻结值，完全挡住箭矢的盾牌会阻止这些效果。两种效果都在服务器通过伤害与领地检查后结算。

爆炸弓在魔法工作台制作，沿用 `special_bows` 研究。普通箭矢命中后会播放爆炸声和粒子，对初次受击者附近的其他存活实体计算溅射伤害与击退。区域保护先按原箭矢射手审批，允许时才追加速度和原版无来源伤害。效果不改变方块。`[explosive-bow]` 的 `explosion-range` 默认为 3。直接射中盔甲架会使用同样的溅射规则。仍在线的射手切换维度后，领地检查沿用该玩家的身份。射手已无法解析时仅播放声音和粒子。

七档喷气靴和装甲喷气靴都在增强工作台组装，钢制推进器也用同一工作台制作。先用充电台充电，穿上后按住潜行。每两刻按 Paper 的公式沿视线水平推进，轻微抬升并清除坠落距离。单次扣 0.075 J，再按 Paper 的规则保留两位小数。松开潜行或电量不足时停止。

七档电动喷气背包和装甲背包可用同一台充电台充电。穿着后按住潜行，每三刻扣 0.08 J 并向上推进，同时可用视线转向。由布和锁链制成的降落伞在潜行时减缓下落。

卷尺在增强工作台制作。潜行右键方块可在当前卷尺上保存起点，右键同维度内的另一个方块可测量三维直线距离，最多显示三位小数。转交卷尺时起点随物品保留。若卷尺带有原 Paper 起点，请在 `<Horizon 世界>/horizon-mod-data/slimefun/paper-worlds.toml` 的 `[worlds]` 表中填写 `"旧 Paper 世界 UUID" = "minecraft:overworld"` 等对应关系；也可以重新选定起点。

在增强工作台制作并放置 GPS 发射器并接入能源网络，为放置者增加 GPS 复杂度。达到 600 后，可以右击目标区块内的固定 GEO 扫描器，或手持便携扫描器右击目标区块中的方块。扫描会显示按维度和生物群系生成的四种有限储量。接通同一区块的 GEO 矿机即可开采小块铀、下界冰和盐，货运可从矿机的输出槽提取。接通油泵并放入空桶即可提取原油，油桶可送往炼油机，燃料桶可用于内燃发电机。

GEO 矿机在方块上方显示待机、需要扫描、正在开采的资源或开采完毕。产物放不进输出槽时，矿机保留现有提示，等待腾出空间。

GPS 标记工具会记录右击方块对应的相邻位置，随后使用玩家下一条聊天消息作为名称。GPS 控制面板可查看自己的在线发射器、复杂度并删除航点。迁移 Paper 航点时运行 `horizon/tools/import-waypoints.py <Paper数据目录> <Slimefun世界数据目录>/waypoints-import --world <Paper世界名>=<Horizon维度>`。使用 PM-Sync 的快照可加 `--server-id`，并用 Paper 世界 UUID 提供对应的映射。玩家须以同一 UUID 加入，导入保留已有同名航点，之后删除的航点不会从同一档案重新加入。

将传送矩阵放在八个 GPS 支柱中间，把个人或共享激活板放在矩阵上即可打开传送菜单。共享激活板会显示矩阵放置者的航点，个人激活板还要求玩家本人是激活板的放置者。便携传送器使用自己的航点，默认在打开菜单时扣除 10 J，并可在配置项 `[portable-teleporter]` 的 `teleportation-cost` 中调整。选择航点后须留在传送源附近，Horizon 会在实际传送时检查出发地与目的地的领地权限。传送矩阵和个人激活板的旧所有者可随 Paper 方块记录恢复。

研究 `elevator` 后可在增强工作台制作电梯踏板。同一列的踏板从底部开始编号，踩上踏板后可在菜单中选择楼层。踏板的放置者可右键打开编辑菜单，用聊天输入带颜色代码的楼层名。`/sf storage restore-chunk` 可从 Paper 方块档案恢复踏板的 `owner` 和 `floor`，重复导入会保留已有改名。传送会经过当前的领地权限检查。

Auto Enchanter 和 Auto Disenchanter 各有 I、II 两档。分别消耗附魔书或普通书，保留工具的耐久和其他组件。附魔机应用该物品支持的更高等级附魔，去附魔机将直接附魔和铁砧惩罚转移到新书。两个档位的每个处理刻分别消耗 9 J 和 24 J，使用 Paper 原始的输入槽 19、20 和输出槽 24、25，支持货运和库存恢复。前置材料 Wither-Proof Obsidian 可在增强工作台制作，放置后抵抗爆炸和凋灵拆除。默认沿用 Paper 的附魔转移规则。

在 `slimefun-server.toml` 的 `[enchantment-machines.AUTO_ENCHANTER]`、`[enchantment-machines.AUTO_ENCHANTER_2]`、`[enchantment-machines.AUTO_DISENCHANTER]` 或 `[enchantment-machines.AUTO_DISENCHANTER_2]` 中可分别配置等级上限、词条数量和忽略 Lore。用 `use-enchant-level-limit`、`use-max-enchants` 及 `use-ignored-lores` 启用对应规则，再设置 `enchant-level-limit`、`max-enchants` 或 `ignored-lores`。Lore 逐行对比字面文本及 Paper 格式码。有人打开机器且遇到适用的超限词条时，第 22 槽显示提示，机器等待。无人查看时，机器跳过该词条，去附魔会保留原物品上的超限等级。附魔两档另可用 `override-existing-enchants-lvl` 允许同等级重附或覆盖成更低等级。物品专用的 `allow-enchanting` 和 `allow-disenchanting` 位于 `[item-settings.<Slimefun ID>]`。依赖 Bukkit 附魔事件的附加插件仍需兼容层。

Auto Anvil 和 Auto Anvil Mk.II 用一份 Duct Tape 修理一件有损耗的物品，分别恢复该物品原型最大耐久的 10% 和 25%，向下取整。修理保留附魔、铁砧惩罚和其他物品组件，两档均用 60 个处理刻，沿用 Paper 四个输入输出槽。增强工作台每次生产两份 Duct Tape，机器通过 Slimefun ID 识别胶带。

Book Binder 以书架为载体，将两本附魔书合成一本。相同附魔等阶时提升一级，等级与冲突检查读取服务器当前的附魔注册表。默认沿用 Paper 上限。可在 `slimefun-server.toml` 的 `[book-binder]` 表调整 `bypass-vanilla-max-level`、`has-custom-max-level` 与 `custom-max-level`。

Hardened Glass 能抵抗爆炸，凋灵仍可直接拆除它。Wither-Proof Glass 和 Wither-Proof Obsidian 也能抵抗凋灵直接拆除。三者放置后随区块保留原 Slimefun 标记，现有 Paper 世界可用 `restore-network` 或 `restore-chunk` 恢复。

Enhanced Furnace I–XI、Reinforced Furnace 和 Carbonado Edged Furnace 使用原版熔炉界面和库存。各档沿用 Paper 的燃料倍率、加速间隔与指定矿石的幸运产量。普通熔炉、高炉和烟熏炉不受这些规则影响。迁移时用 `/sf storage restore-network` 恢复世界中原有熔炉的 Slimefun 标记，燃料、库存和烹饪状态保留在原版存档中。

灌魔磁铁在魔法工作台制作。研究 `infused_magnet` 后，库存中携带磁铁并按住 Shift，每八刻吸取附近可拾取的物品，默认半径六格。用它和古代符文可在古代祭坛合成灌魔漏斗。漏斗把附近物品移到入口，保留原版漏斗库存、红石及货运行为。默认半径 3.5 格。`[infused-hopper]` 支持 `radius`、`silent`、`toggleable-with-redstone`，`[infused-magnet]` 支持 `pickup-radius`。

全息投影器放置后显示可编辑文字。只有放置者能打开编辑器，在聊天中输入原 Minecraft 颜色代码，或用时钟按每次 0.1 格调整高度。迁移 Paper 投影器时可用 `/sf storage restore-network`，沿用所有者 UUID、文字和偏移，并接管原位置的投影实体。

在 `slimefun-server.toml` 的 `[URID]` 表中设置 `enable-tickers = false` 并重启，可停止方块计时器物品的配方、指南展示、新放置和手动使用。已有标记方块按 Paper 的旧存档规则保留其计时处理、库存和可恢复的掉落。电动熔炉、电动磨石和电动锭厂会跳过 Paper 启动后追加的处理配方。已有的电动锭粉碎机仍可将原版铜锭和金锭磨成粉。普通货运节点、发电机和手动设备仍可制作和放置。

服主也可以关闭指定物品，或仅在某个维度关闭。配置在重启时读取。示例中的货运管理器全服关闭，高级货运输出节点仅在下界关闭。

```toml
[item-settings.CARGO_MANAGER]
enabled = false

[worlds."minecraft:the_nether".items]
CARGO_NODE_OUTPUT_ADVANCED = false
```

世界表内的 `enabled = false` 会同时关闭该世界的 Slimefun 指南入口。用 `slimefun-enabled = false` 可保留指南入口并关闭该世界所有原版 Slimefun 物品的手动使用。分组目录按当前世界过滤，搜索沿用 Paper 的全服物品范围。全服关闭某件物品会停掉它自身的计时器，包括已有的货运管理器或能源调节器。仅按世界关闭会保留旧方块的计时和库存。已经铺设的普通货运节点和被动能源构件仍由运行中的管理器或调节器识别。关闭 Slimefun 的连锁甲、鞘翅等原版物品配方时，原版物品本身仍可正常使用。

导入 Paper 配置时，可给 `horizon/tools/import-block-storage.py` 附加 `--item-config <Paper 的 Items.yml>` 和 `--world-settings <Paper 的 world-settings 目录>`。自定义世界使用相同的 `--map-world` 参数。将生成的 `settings-item-availability.toml` 合并到 `slimefun-server.toml` 后重启。

将货运输入或输出节点挂到库存方块侧面，再右键配置频道。输入和高级输出节点可放置白名单或黑名单物品。输入节点另可启用轮询和智能填充。空白名单不转移物品。货运支持普通库存和已移植的电动机器，每次按实际源、目标库存位置检查 Horizon 的库存转移许可。目标只能接收部分物品时，源端保留余量。

向熔炉、高炉及烟熏炉送货时，可燃原木优先进入原料槽，原料槽无法接收时进入燃料槽。木板等非原木燃料进入燃料槽。已经启动的电网先保留发现范围内的调节器位置，后加的调节器显示冲突。拆除旧调节器后，仍连通的调节器可在下一轮发现中接管。

使用 `/sf research block_placer` 解锁方块放置器，合成后放置发射器并从侧面输入红石信号。它使用实际库存中选中的物品，两 tick 后在朝向的空格放置。库存打开时暂停开始新操作。发射器下方有漏斗以及原版潜影盒使用普通发射行为。

Paper 存档可用 `/sf storage restore-network` 或分块恢复命令恢复方块放置器及其属主。属主离线时仍按 UUID 检查建造许可。早期未保存属主的记录保留 Paper 的行为。可在 `[block-placer]` 的 `unplaceable-blocks` 配置额外的材料名单，覆盖默认不可破坏材料名单。
