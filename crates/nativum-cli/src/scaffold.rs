//! `nativum init`.

use std::path::Path;

use anyhow::{bail, Result};

pub fn init(name: &str) -> Result<()> {
    let dir = Path::new(name);
    if dir.exists() {
        bail!("{} already exists", dir.display());
    }
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::create_dir_all(dir.join("tests"))?;
    std::fs::write(dir.join("app.json"), app_json(name))?;
    std::fs::write(dir.join("src/app.native"), APP_NATIVE)?;
    std::fs::write(dir.join("src/core.json"), CORE_JSON)?;
    std::fs::write(dir.join("tests/increment.json"), TEST_JSON)?;
    std::fs::write(dir.join("README.md"), readme(name))?;
    println!("created {name}/");
    println!("  nativum check {name}");
    println!("  nativum render {name} --out {name}/preview.png");
    println!("  nativum preview {name}");
    Ok(())
}

fn app_json(name: &str) -> String {
    format!(
        r#"{{
  "name": "{name}",
  "display_name": "{name}",
  "version": "0.1.0",
  "window": {{
    "title": "{name}",
    "width": 360,
    "height": 280
  }},
  "view": "src/app.native",
  "core": "src/core.json"
}}
"#
    )
}

const APP_NATIVE: &str = r#"<column padding="24" gap="20" background="background" grow="1">
  <text size="heading" foreground="text_muted">counter</text>
  <text size="display" text-alignment="center">{count}</text>
  <row gap="8" main="center" cross="center">
    <button variant="secondary" on-press="decrement" label="decrement">-</button>
    <button variant="primary" on-press="increment" label="increment">+</button>
  </row>
  <row main="center">
    <button on-press="reset">Reset</button>
  </row>
</column>
"#;

const CORE_JSON: &str = r#"{
  "initial": { "count": 0 },
  "update": {
    "increment": { "count": "count + 1" },
    "decrement": { "count": "count - 1" },
    "reset": { "count": "0" }
  }
}
"#;

const TEST_JSON: &str = r#"{
  "steps": [
    { "press": "increment" },
    { "press": "increment" }
  ],
  "assert": { "count": 2 }
}
"#;

fn readme(name: &str) -> String {
    format!(
        "# {name}

A nativum app. Three files of truth: `src/app.native`, `src/core.json`, `app.json`.

```sh
nativum check
nativum render --out preview.png
nativum test
nativum preview
```
"
    )
}
