# LingCode Extensions

This directory contains extensions for LingCode that are largely maintained by the LingCode team. They currently live in the LingCode repository for ease of maintenance.

If you are looking for the LingCode extension registry, see the [`zed-industries/extensions`](https://github.com/zed-industries/extensions) repo.

## Structure

Currently, LingCode includes support for a number of languages without requiring installing an extension. Those languages can be found under [`crates/languages/src`](https://github.com/Xavierhuang/lingcode_window_IDE/tree/main/crates/languages/src).

Support for all other languages is done via extensions. This directory ([extensions/](https://github.com/Xavierhuang/lingcode_window_IDE/tree/main/extensions/)) contains some of the officially maintained extensions. These extensions use the same [zed_extension_api](https://docs.rs/zed_extension_api/latest/zed_extension_api/) available to all [LingCode Extensions](https://lingcode.dev/extensions) for providing [language servers](https://lingcode.dev/docs/extensions/languages#language-servers), [tree-sitter grammars](https://lingcode.dev/docs/extensions/languages#grammar) and [tree-sitter queries](https://lingcode.dev/docs/extensions/languages#tree-sitter-queries).

You can find the other officially maintained extensions in the [zed-extensions organization](https://github.com/zed-extensions).

## Dev Extensions

See the docs for [Developing an Extension Locally](https://lingcode.dev/docs/extensions/developing-extensions#developing-an-extension-locally) for how to work with one of these extensions.
