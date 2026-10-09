# Slimefun 5.0.1 玩家档案同步

本功能用于 Paper 26.2 / Java 25、PM-Sync + HuskSync 4.0.0 的 hub/survival 网络。Slimefun 继续按 GPLv3 分发；不捆绑 PM-Sync/HuskSync 的实现或凭据。单服默认沿用原有保存方式。

## 覆盖表

| 数据 | 位置与同步方式 | 边界 |
| --- | --- | --- |
| 研究解锁 | `slimefun` 外部 section；NamespacedKey | 首迁保留旧数字 ID，包括重复 ID 173 对应的研究；缺失附属插件的研究键/数字 ID 原样保留 |
| 背包/冰箱 | 同 section，所有背包 ID、大小、每一槽的 Paper `serializeAsBytes()` | 包含原生物品组件、ItemMeta/PDC；空槽明确编码，保留稀疏 ID，分配新 ID 不覆盖旧包 |
| 首次指南状态 | 同 section，`guideIssued` | 首次全网完整加载后才发；迁移已玩过/有记录的账号不补发；各后端首次 join 不重复发 |
| 手选语言、研究烟花/动画、StatusEffect | 玩家实体 PDC，由 HuskSync 捕获 | 与背包、研究进入同一份 PM-Sync envelope；Slimefun 不另存一份 PDC |
| 指南模式 | 指南 ItemMeta，由 HuskSync 捕获物品 | 原代码没有单独的玩家指南模式字段 |
| 指南浏览历史 | 内存会话 | 原版不持久化，换服重建；核心没有收藏/书签持久化字段 |
| GPS 定位点 | 本服 `data-storage/Slimefun/player-sync/<server-id>/waypoints/<UUID>.yml` | 世界名、UUID、key、坐标；从本服旧 waypoints 文件初读；不在全局快照。未挂载世界的记录保留；不按同名世界替换 UUID |
| 方块机器、电网、物流、机器人、世界存储 | 原有本服世界数据 | 不跨服覆盖，不进入玩家同步 |
| 附属插件自建 SQL/文件、deprecated `getConfig()` 扩展字段 | 不在本协议内 | 需各插件独立 participant；网络 profile 的 deprecated 原始文件 API 抛异常，避免绕过权威快照 |

网络模式暂限**本人背包**，包括冰箱自动消耗、打开和升级。原版借用别人的背包需要另行实现每背包的跨服 SQL 锁；玩家 owner 锁不能保护别人的包。单服行为不受此限制。第三方附属插件直接改 `PlayerData`/Inventory 必须先核对 profile 当前有效并在主线程操作；不能把本协议推定为所有附属插件的兼容保证。

## 配置及顺序

两后端同时部署本版本，并分别配置 Slimefun `config.yml`：

```yaml
player-sync:
  enabled: true
  server-id: survival # hub 后端改为 hub，必须与 PM-Sync 一致
```

PM-Sync 配置启用 `slimefun` 外部 section，softdepend Slimefun，并验证下方服务。Slimefun 不直接连接 SQL；数据库 `pigeonmoe`、Redis、fencing/session/token/history 由 PM-Sync 管理。凭据只在部署端填写。缺少 PM-Sync、缺失 section、损坏/超限数据、初始化失败或超时都会保持锁定，禁止自动退回旧文件。

1. 目标 `beginLoad` 冻结并使旧缓存失效；PM-Sync 获得 SQL owner 后加载 Husk 数据和 `slimefun` section。仅**完全没有全局快照**时允许受控 `bootstrap` 读取旧档；保护的旧账号先在 survival 迁移。已有全局快照缺 section 必须拒绝，不能初始化空档。
2. `apply` 严格校验完整 section、全部背包物品和本服 GPS 后安装内存；future 成功仍冻结。Shulker、PMA 和其他 participants 全部完成，PM-Sync 才调用 `onSyncLoaded`，随后开放交互。
3. 源端 `prepareTransfer` 等待已付费研究动画结束（最长约 5 秒），返回 false 时 PM-Sync 在 15 秒窗口重试；成功关闭本插件背包/菜单，保存本服 GPS 并冻结。不会关闭无关插件的空鼠标菜单。实际切服的全部 GUI 关闭由 PM-Sync 负责。
4. `capture` 主线程抓完整外部档案，PM-Sync 与同一时刻的 inventory/PDC/health/recipes 等写入 **同一个 SQL 快照事务**。无需 rsync playerfiles。普通 checkpoint 在捕获后 `abortTransfer` 恢复；真正切服仅 SQL commit/release 成功后 `sourceCommitted` 清缓存，随后代理切服。

普通 quit 后保留 source 缓存，允许 PM-Sync 等待 Shulker/研究排空后再抓取离线 Player 对象。失败不清缓存、不清 dirty、不写空档。旧 Player 身份和迟到的异步加载不能重新安装旧档。网络模式关闭 legacy 玩家 autosave/退出全局写回，世界 autosave 保持原行为。GPS 文件使用同目录临时文件、force 和原子替换；不承诺与全局 SQL 跨存储原子提交，二者的数据归属本来独立。

## API v1

Bukkit `ServicesManager` 注册服务的 FQCN 为 `moe.pigeon.slimefun.api.PlayerSyncBridge`，provider 为公开的 `PlayerSyncService`。协调方按服务 FQCN 扫描并反射调用，无需链接或复制 GPL API 源码。

```java
int apiVersion(); // 1
void beginLoad(Player player);
boolean blocked(Player player);
boolean prepareTransfer(Player player);
byte[] capture(Player player);
CompletableFuture<byte[]> bootstrap(Player player);
CompletableFuture<Boolean> apply(Player player, byte[] snapshot);
void onSyncLoaded(Player player);
void abortTransfer(Player player);
void sourceCommitted(Player player);
```

所有入口要求主线程；bootstrap/apply 的文件读取异步，Bukkit 物品/档案安装回主线程，只有安装完成才完成 future。协调方必须异步组合 future，不能在主线程 join。section 使用 SFP1、有 owner UUID、SHA-256 校验和和严格边界：8 MiB/档案、4096 个背包、16384 个研究键和旧 ID、1 MiB/物品，超限拒绝，不截断。校验和用于损坏检测，可信来源与会话隔离由 SQL fencing 负责。

## 升级与回退

部署由服务器聊天统一执行，维护者不自行启动服务器或修改世界。先停写并备份两后端旧 JAR、配置、`data-storage/Slimefun`、原生 playerdata 及 PM-Sync 数据库快照/history；两服同批升级并校验 SHA。不要热重载；不要混跑网络模式与 legacy 模式。

网络运行后旧 `Players/*.yml` 不再更新，因此**不能**仅换回旧 JAR/关闭 enabled，否则会恢复陈旧研究和背包。回退须停写全网，同时回到同一备份时刻的 SQL、玩家数据、外部档案和配置，或先由另行实现并验收的导出工具从最新权威 snapshot 导出；本版本不提供反向导出。上线前核对旧账号/新账号、背包组件和稀疏 ID、语言、研究、两服各自 GPS、首次指南、快速连退、研究中切服、事务失败/超时、服务器重启后恢复，以及附属插件边界。

## 验证

新增 MockBukkit/纯编解码测试覆盖加载门禁、显式解冻、注册接口、旧会话/迟到加载、损坏/超限/错 owner、严格初迁、重复旧研究 ID、未知附属研究保留、全部背包槽及 ItemMeta/PDC、稀疏 ID、研究排空、checkpoint 恢复、GPS 隔离和未挂载世界、借包限制、全网一次指南。MockBukkit 的物品二进制格式是模拟实现，不能替代真实 Paper 的全组件检查。

本次只执行离线编译与测试，未在本机启动游戏/服务器，未访问远端或修改世界。之前 5.0.0-SNAPSHOT 的 555 物品 Paper 探针记录只适用于旧构建；5.0.1 的真实 SQL/双后端/玩家切服验收由服务器聊天另行记录，不能把旧记录当作本版已通过。

### 部署端补充（2026-10-09）

服务器维护聊天已报告正式 5.0.1 + PM-Sync 0.1.2 + HuskSync 4.0.0 的 hub/survival 实服验收：研究解锁、带自定义物品数据的完整背包容器编码、实际 SQL 快照解析、一次性指南，以及双后端正常重启/公网重登后的背包恢复通过。详见 [验收补充](verification/26.2-5.0.1-network-acceptance.json)。这些是部署端报告的检查，维护者未独立重跑；GPS 未人为创建/测试，机器、全部物品组件、附属插件和所有故障/回退路径仍不能据此视为完成验收。原始构建验证记录及正式二进制不变。
