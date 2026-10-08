# Slimefun5 国际化

## 使用

`options.enable-translations: true` 开启翻译。`options.language` 可设置为 `en`、`zh-CN` 或 `zh-TW` 等内置语言。默认开启 `options.auto-detect-language`：未手动选择语言的玩家使用客户端语言；在指南里选定的语言优先于客户端设置；无法识别时使用服务器默认语言。

支持 `zh_CN`、`zh-Hans`、`zh-SG` 等简中别名，以及 `zh_TW`、`zh-Hant`、`zh-HK`、`zh-MO` 等繁中别名。其他语言仍使用上游资源，缺失物品译名回退英文或原始名称。

物品翻译位于 `languages/<语言>/items.yml`，按持久化物品 ID 索引。指南分类、搜索、配方材料与结果显示玩家所选语言；搜索同时接受本地化名称、英文名称和物品 ID。

翻译作用于指南的物品副本，保留内部 ID、配方、材质、纹理和持久化数据。现有背包物品、机器输出与作弊获取物品继续使用规范模板。仅替换精确匹配的静态描述，不覆盖电量、背包编号、刷怪笼类型等动态或功能性 lore。机器实时界面、所有动态描述及实体物品按玩家语言显示尚未全部迁移，不能把当前开发构建视为全量中文化完成。

## 译文来源与许可

原有消息、分类、配方说明、研究和资源译文来自 Slimefun4 上游及其翻译贡献者，本分支补齐繁中缺项并修复占位符。

新增物品简中译名及可精确对齐的静态描述取自 [SlimefunGuguProject/Slimefun4](https://github.com/SlimefunGuguProject/Slimefun4)，固定来源提交 `3b8b3caff03ccc6a0ecb1b5a3b5efeaacbec36c3` 的 [SlimefunItems.java](https://github.com/SlimefunGuguProject/Slimefun4/blob/3b8b3caff03ccc6a0ecb1b5a3b5efeaacbec36c3/src/main/java/io/github/thebusybiscuit/slimefun4/implementation/SlimefunItems.java)。感谢该项目的作者和翻译贡献者。译文作为 GPLv3 派生内容沿用本仓库 LICENSE，未引入其机器实现或数据迁移逻辑。

新增物品繁中译文由上述简中内容经 OpenCC `s2twp` 转换产生；现有繁中消息保留原译。繁中用语仍需要人工审校。物品语义或数值与来源分支不一致时不导入对应描述。

## 验证

测试检查两种中文覆盖英文资源键并保留 `%placeholder%`，检查客户端识别、手动选择优先、未知语言回退，以及翻译副本不修改原模板和物品 ID。真实 Paper 探针另外验证中文资源加载和物品识别。

此外为运行时生成的末影护符、原版配方条目和符文补齐了译名；共 559 个资源条目，覆盖当前默认注册的 555 个物品。
