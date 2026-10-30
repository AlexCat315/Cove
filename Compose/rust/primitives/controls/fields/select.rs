//! Shared select values and keyboard shortcut model from Fields/Select.swift.
//! The popup-backed Select view is still being ported.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyboardShortcutPlatform {
    MacOS,
    Windows,
    Linux,
    Other,
}

impl KeyboardShortcutPlatform {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") { Self::MacOS }
        else if cfg!(target_os = "windows") { Self::Windows }
        else if cfg!(target_os = "linux") { Self::Linux }
        else { Self::Other }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyboardShortcutModifier {
    Primary,
    Command,
    Control,
    Option,
    Shift,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyboardShortcut {
    pub modifiers: Vec<KeyboardShortcutModifier>,
    pub key: String,
}

impl KeyboardShortcut {
    pub fn new(key: impl Into<String>, modifiers: impl IntoIterator<Item = KeyboardShortcutModifier>) -> Self {
        Self { modifiers: modifiers.into_iter().collect(), key: key.into() }
    }

    pub fn primary(key: impl Into<String>) -> Self {
        Self::new(key, [KeyboardShortcutModifier::Primary])
    }

    pub fn primary_shift(key: impl Into<String>) -> Self {
        Self::new(key, [KeyboardShortcutModifier::Primary, KeyboardShortcutModifier::Shift])
    }

    pub fn display_string(&self) -> String {
        self.display_string_for(KeyboardShortcutPlatform::current())
    }

    pub fn display_string_for(&self, platform: KeyboardShortcutPlatform) -> String {
        let caps = self.caps(platform);
        match platform {
            KeyboardShortcutPlatform::MacOS => caps.concat(),
            KeyboardShortcutPlatform::Windows | KeyboardShortcutPlatform::Linux | KeyboardShortcutPlatform::Other => caps.join("+"),
        }
    }

    pub fn caps(&self, platform: KeyboardShortcutPlatform) -> Vec<String> {
        let mut resolved = Vec::new();
        for modifier in &self.modifiers {
            let modifier = if *modifier == KeyboardShortcutModifier::Primary {
                if platform == KeyboardShortcutPlatform::MacOS { KeyboardShortcutModifier::Command }
                else { KeyboardShortcutModifier::Control }
            } else { *modifier };
            if !resolved.contains(&modifier) { resolved.push(modifier); }
        }
        let mut caps: Vec<String> = resolved.into_iter().map(|modifier| match platform {
            KeyboardShortcutPlatform::MacOS => match modifier {
                KeyboardShortcutModifier::Primary | KeyboardShortcutModifier::Command => "⌘",
                KeyboardShortcutModifier::Control => "⌃",
                KeyboardShortcutModifier::Option => "⌥",
                KeyboardShortcutModifier::Shift => "⇧",
            },
            KeyboardShortcutPlatform::Windows | KeyboardShortcutPlatform::Linux | KeyboardShortcutPlatform::Other => match modifier {
                KeyboardShortcutModifier::Primary | KeyboardShortcutModifier::Control => "Ctrl",
                KeyboardShortcutModifier::Command => "Meta",
                KeyboardShortcutModifier::Option => "Alt",
                KeyboardShortcutModifier::Shift => "Shift",
            },
        }.to_string()).collect();
        caps.push(self.key.clone());
        caps
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SelectOption<Value> {
    pub value: Value,
    pub label: String,
    pub is_enabled: bool,
}

impl<Value> SelectOption<Value> {
    pub fn new(value: Value, label: impl Into<String>, is_enabled: bool) -> Self {
        Self { value, label: label.into(), is_enabled }
    }

    pub fn id(&self) -> &Value { &self.value }
}
