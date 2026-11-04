//! Code of one language inside a file of another: the `<script>` of a Svelte component is TypeScript or
//! JavaScript. The file's grammar leaves such text unparsed, so it is found by the node tables
//! ([`Spec::embedded`](crate::lang::Spec)) and analysed as the language it names, with the line numbers of the file.

use tree_sitter::Node;

use crate::lang::{Embed, Lang};
use crate::parse::{parse, text};

/// A stretch of the file in another language.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Region {
    pub lang: Lang,
    pub start: usize,
    pub end: usize,
    /// The row (from 0) the region starts on: line `n` of its text is line `n + row` of the file.
    pub row: usize,
}

/// The embedded regions of `source`, in order; none for a language that embeds nothing.
pub(crate) fn regions(lang: Lang, source: &str) -> Vec<Region> {
    let embeds = lang.spec().embedded;
    if embeds.is_empty() {
        return Vec::new();
    }
    let tree = parse(lang, source);
    let mut out = Vec::new();
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        match embeds.iter().find(|e| e.container == node.kind()) {
            Some(embed) => out.extend(region_of(node, embed, source)),
            None => {
                let children: Vec<_> = node.children(&mut cursor).collect();
                stack.extend(children.into_iter().rev());
            }
        }
    }
    out
}

fn region_of(container: Node, embed: &Embed, source: &str) -> Option<Region> {
    let mut cursor = container.walk();
    let content = container
        .children(&mut cursor)
        .find(|n| n.kind() == embed.content);
    content.map(|content| Region {
        lang: language_of(container, embed, source),
        start: content.start_byte(),
        end: content.end_byte(),
        row: content.start_position().row,
    })
}

/// The language the container's `lang` attribute names, or the embed's fallback.
fn language_of(container: Node, embed: &Embed, source: &str) -> Lang {
    let [attribute, name, value] = embed.attribute_kinds;
    let mut stack = vec![container];
    let mut cursor = container.walk();
    while let Some(node) = stack.pop() {
        if node.kind() == attribute {
            let children: Vec<_> = node.children(&mut cursor).collect();
            let named = children.iter().find(|n| n.kind() == name);
            if named.is_some_and(|n| text(*n, source.as_bytes()) == embed.attribute) {
                return value_lang(&children, value, embed, source);
            }
        }
        stack.extend((0..node.child_count()).filter_map(|i| node.child(i)));
    }
    embed.fallback
}

/// The language the attribute's value names; the value may sit inside a quoted-value node.
fn value_lang(attribute: &[Node], value_kind: &str, embed: &Embed, source: &str) -> Lang {
    let mut stack: Vec<Node> = attribute.to_vec();
    while let Some(node) = stack.pop() {
        if node.kind() == value_kind {
            let wanted = text(node, source.as_bytes()).trim().to_ascii_lowercase();
            return embed
                .langs
                .iter()
                .find(|(v, _)| *v == wanted)
                .map_or(embed.fallback, |(_, lang)| *lang);
        }
        stack.extend((0..node.child_count()).filter_map(|i| node.child(i)));
    }
    embed.fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::typescript("<script lang=\"ts\">\nlet a = 1;\n</script>\n", Lang::TypeScript)]
    #[case::typescript_long("<script lang='typescript'>let a = 1;</script>", Lang::TypeScript)]
    #[case::javascript("<script>let a = 1;</script>", Lang::JavaScript)]
    #[case::other_attribute("<script context=\"module\">let a = 1;</script>", Lang::JavaScript)]
    #[case::unknown_language("<script lang=\"coffee\">a = 1</script>", Lang::JavaScript)]
    fn the_lang_attribute_picks_the_language(#[case] source: &str, #[case] want: Lang) {
        let found = regions(Lang::Svelte, source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].lang, want);
        assert!(source[found[0].start..found[0].end].contains("a ="));
    }

    #[test]
    fn regions_come_in_order_with_the_row_they_start_on() {
        let source = "<script context=\"module\">\nlet a = 1;\n</script>\n<p>hi</p>\n<script>\nlet b = 2;\n</script>\n";
        let found = regions(Lang::Svelte, source);
        assert_eq!(found.iter().map(|r| r.row).collect::<Vec<_>>(), vec![0, 4]);
        assert!(source[found[1].start..found[1].end].contains("let b"));
    }

    #[test]
    fn a_language_that_embeds_nothing_has_no_regions() {
        assert!(regions(Lang::Go, "package p\n").is_empty());
    }

    #[test]
    fn markup_and_style_are_not_regions() {
        assert!(regions(
            Lang::Svelte,
            "{#if x}<p>hi</p>{/if}\n<style>p { color: red }</style>\n"
        )
        .is_empty());
    }

    #[test]
    fn an_empty_script_is_an_empty_region() {
        let source = "<script></script>\n<p>hi</p>\n";
        let found = regions(Lang::Svelte, source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].start, found[0].end);
        assert!(crate::complexity::complexity(Lang::Svelte, source).is_empty());
    }

    #[test]
    fn a_lang_attribute_without_a_value_means_the_fallback() {
        let source = "<script lang>let a = 1;</script>";
        let found = regions(Lang::Svelte, source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].lang, Lang::JavaScript);
    }
}
