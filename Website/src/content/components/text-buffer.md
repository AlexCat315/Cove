# TextBuffer

`TextBuffer` 是不可变文本值。字符索引使用[扩展字素簇](https://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries)：组合重音字母、带肤色的 ZWJ emoji、国旗各计一个字符，CRLF 也计一个字符和一个换行。

```rust
use cove_ui::prelude::*;

fn characters() {
    let text = TextBuffer::new("e\u{301}👩🏽‍💻🇨🇳\r\n界");
    assert_eq!(text.character_count(), 5);
    assert_eq!(text.line_count(), 2);
    assert_eq!(text.character_at(1).as_deref(), Some("👩🏽‍💻"));
    assert_eq!(text.substring(1..3), "👩🏽‍💻🇨🇳");

    let joined = TextBuffer::new("e").insert("\u{301}", 1);
    assert_eq!(joined.character_count(), 1);
    assert_eq!(joined.substring(0..1), "e\u{301}");
}
```

`substring`、`insert`、`delete`、`replace`、`line_range` 和 `character_at` 使用同一套字素索引。`character_at` 返回 `Option<String>`，以容纳包含多个 Unicode 标量的一个字符。插入或删除使两侧文字合并时，缓冲区会重建受影响的字素边界；保留的旧快照不改变。

| 坐标 | API | 用途 |
| --- | --- | --- |
| 字素索引 | `character_count`、`substring`、`line_and_column` | 光标、选区和编辑 |
| UTF-8 字节 | `utf8_length`、`utf8_offset_for_character_index`、`character_index_for_utf8_offset` | 语法配色、编辑增量与文件数据 |
| UTF-16 单元 | `utf16_length`、`utf16_offset_for_character_index` | 平台输入与协议坐标 |

`character_index_for_utf8_offset` 将字素内部的偏移向后取整到字素末尾。`utf8_offset_for_utf16_offset` / `utf16_offset_for_utf8_offset` 保留标量边界，供协议表达组合字符内部的位置；用于光标时还需转换到字素边界。平台输入客户端会完成这一步。

`parser_point` 按 LF 字节计算行号，`point_for_utf8_offset` 按编辑器换行计算行号，两者在 CRLF 内部或单独 CR 处可以不同。

复制缓冲区和保存撤销快照共享不可变 rope。`character_count` 读取缓存计数；索引转换沿树查找。编辑保留未受影响的叶节点；单个超长字素不会为满足叶大小而被拆开。需要完整字符串时调用 `string_value`。

这是索引语义的破坏性修正：原来使用 Unicode 标量索引的调用方需要改用字素索引，不能直接传入 `chars().count()` 的结果。
