# 依赖兼容补丁

`src/main/java/io/github/bakedlibs/dough/items/ItemUtils.java` 来自 `Slimefun/dough@cb22e71335` 的同名类，沿用 MIT 许可，完整声明位于源文件及 `LICENSES/dough-MIT.txt`。

26.2 的版本标识不是旧 Dough 预期的语义版本。原来的静态 NMS 物品名称适配器在加载时抛出版本识别错误，导致原版材料显示为 unknown。本分支仅将名称读取切换为 Paper 的公共 `ItemStack.getI18NDisplayName()`，保留其余堆叠、消耗和耐久逻辑。打包时排除依赖 JAR 的同名类，使用本地兼容实现；最终仍重定位至 Slimefun 私有依赖包，避免影响服务器的其他插件。

该补丁应随上游 Dough 的新版本重新评估。不要删除 MIT 声明，也不要重新启用依赖 JAR 的旧实现。
