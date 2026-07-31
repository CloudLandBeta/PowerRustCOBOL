// SPDX-License-Identifier: Apache-2.0

//! Built-in mesh specialists: prompt-level routing and system-prompt
//! composition for the IDE's single-agent entry points. Pure prompt logic —
//! the transport is [`crate::rig_transport`] (Rig migration, phase 4; the
//! hand-rolled HTTP orchestrator that used to live beside this is deleted).

/// Everything one mesh request needs. The host composes the system prompt /
/// skills / context (spec 025 R14/R21/R2 contract) — the mesh never invents
/// or drops them.
pub struct MeshRequest {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    /// How long `api_key` has been on file, in days — carried alongside the
    /// credential so a 401 can say how stale it is. `None` when the host
    /// recorded no date for it.
    pub key_age_days: Option<i64>,
    pub endpoint: String,
    /// Explicit target specialist (FormsDesigner, EventBinder, CodeGenerator).
    /// If None, the mesh routes based on the user prompt.
    pub specialist: Option<String>,
    /// Full system prompt composed by the host. Empty ⇒ the routed
    /// specialist's built-in preamble is used instead.
    pub system_prompt: String,
    /// Reference material (skills).
    pub skills: String,
    /// Per-request context (form snapshot, current code…) appended to the
    /// final user message.
    pub context: String,
    /// Prior conversation turns as (role, content).
    pub history: Vec<(String, String)>,
    pub user_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Include full request/response bodies in the returned trace.
    pub verbose: bool,
}

pub struct Specialist {
    pub name: String,
    pub system_prompt: String,
}

/// Compose the effective system prompt: built-in mesh routes combine the
/// host's prompt with the specialist's preamble; named project agents use
/// their complete host-supplied prompt without a foreign preamble.
pub fn compose_system_prompt(host_prompt: &str, specialist: Option<&Specialist>) -> String {
    match (host_prompt.trim(), specialist) {
        ("", Some(specialist)) => specialist.system_prompt.clone(),
        ("", None) => String::new(),
        (host, Some(specialist)) => format!("{host}\n\n{}", specialist.system_prompt),
        (host, None) => host.to_string(),
    }
}

const BASE_PROTOCOL: &str = r##"
You can do exactly four things, and nothing else:
1. **Deploy a new control** onto the current form.
2. **Edit any property of any existing control.**
3. **Generate a COBOL event-handler** for a control's event.
4. **Create a common procedure** (shared COBOL routine callable from handlers).

## How you must respond
Reply with **one JSON object and nothing else** — no prose outside the JSON,
wrapped in a single fenced block:
```json
{ "operations": [ /* zero or more operation objects, applied in order */ ] }
```

Each element of `operations` is exactly one of:
- `{ "op": "deploy_control", "control_type": "Button", "id": "SAVE-BUTTON", "properties": { "Caption": "Save", "X": 300, "Y": 240, "Parent": "TabControl-1", "Tab": 0 } }`
- `{ "op": "set_property", "control_id": "TOTAL-LABEL", "key": "ForegroundColor", "value": "#008000" }`
- `{ "op": "generate_event_handler", "control_id": "SAVE-BUTTON", "event": "onClick", "code": "       ENVIRONMENT DIVISION..." }`
- `{ "op": "create_procedure", "name": "VALIDATE-INPUT", "code": "       ENVIRONMENT DIVISION..." }`
- `{ "op": "message", "message": "I noticed the property you asked for does not exist." }`

If the request cannot be expressed with these operations, or is a plain question, return `{ "operations": [ { "op": "message", "message": "..." } ] }`. Never invent an operation type.

## Rules
- **Absolute Positioning**: `X` and `Y` coordinates are ALWAYS absolute to the form canvas, even when parenting to a `TabControl` or `Panel`. You MUST calculate `X` and `Y` by adding the parent's `X` and `Y` coordinates to your desired relative position. If you use relative coordinates, the control will be drawn outside its parent and clipped.
- **Only act on what the developer asked.** Do not add, remove, or change anything they did not request.
- **Use only what exists.** The `control_id` MUST be a control in the CONTEXT. Property keys MUST come from that control's valid-keys list.
- **All COBOL and all identifiers are English.** Control ids and procedure names are UPPER-CASE with hyphens.
- **Handler / procedure code follows RustCOBOL, not plain COBOL-85**. Emit the nested-program **body** from `ENVIRONMENT DIVISION` down. Never write `IDENTIFICATION DIVISION`, `PROGRAM-ID`, `GOBACK`, or `END PROGRAM`. Interact with controls using COBOL-2002-style inline members only: read/write properties as `<control>::<property>` and invoke methods as `<control>::<method>(<parameters>)` (e.g., `SET Button-1::Caption TO "Hi".`, `DataGrid-1::RefreshBinding().`). Do not use `CALL` or legacy `INVOKE Control "Method" USING ...` for control methods, property get/set, chart actions, data bindings, REST actions, SQL actions, or IndexedFile actions. Fixed-format indentation: divisions/sections at column 8, statements at column 12.
- **Deploy** only control types listed in the CONTEXT legend. Keep the change-set **minimal**.
- **Do not invent variables.** Never invent array variables like `WS-LINECHART-1-TABLE` or `WS-LINECHART-1-COUNT` for charts. If you must manipulate a chart in code, use inline chart methods such as `LineChart-1::Clear()`, `LineChart-1::AddPoint(label, value)`, and `LineChart-1::Refresh()`, or rely on the `DataSource` property. Only use variables explicitly present in the CONTEXT.
"##;

/// The three built-in mesh specialists.
pub const BUILTIN_SPECIALISTS: [&str; 3] = ["FormsDesigner", "CodeGenerator", "EventBinder"];

impl Specialist {
    /// The built-in specialist with this exact name, if any. Named project
    /// agents (anything else) get no built-in preamble.
    pub fn builtin(name: &str) -> Option<Self> {
        BUILTIN_SPECIALISTS
            .contains(&name)
            .then(|| Self::new(name.to_string()))
    }

    pub fn new(name: impl Into<String>) -> Self {
        let name_str = name.into();
        let prompt = match name_str.as_str() {
            "FormsDesigner" => format!("You are the **PowerRustCOBOL Forms Designer Agent**. Your primary focus is on deploying controls and setting properties to build the visual UI layout.\nYou MUST follow the `rustcobol-desktop-form-design` skill whenever creating, modifying, validating, or reorganizing forms: default 14px typography, black readable foreground for buttons/tabs/input controls, 15px corner radius where supported, theme-aware colors and shadows, correct label/input alignment, valid tab/container ownership, preservation of existing controls/events/bindings, and atomic structured operations.\nIf the user asks a conversational question (like syntax, guidelines, style) or you need more information, answer using the `message` operation inside the `operations` array.\n{}", BASE_PROTOCOL),
            "EventBinder" => format!("You are the **PowerRustCOBOL Event Binder Agent**. Your primary focus is on generating COBOL event handlers for UI controls.\nIf the user asks a conversational question (like syntax, guidelines, style) or you need more information, answer using the `message` operation inside the `operations` array.\n{}", BASE_PROTOCOL),
            _ => "You are the **PowerRustCOBOL Code Generator Agent**. Your primary focus is on creating complex COBOL procedures and backend business logic.
When you write COBOL code, you MUST output it inside a markdown code block:
```cobol
       ENVIRONMENT DIVISION...
```
Do not output JSON.
If the user asks a conversational question (like syntax, guidelines, style) or you need more information, simply reply with text (do NOT include a ```cobol code block). Engage in conversation to fill any gaps of understanding.".to_string(),
        };

        Self {
            name: name_str,
            system_prompt: prompt,
        }
    }
}

/// Route a developer prompt to a built-in specialist by domain keywords, in
/// all six PowerRustCOBOL UI languages (English, Spanish, Portuguese,
/// Japanese, Chinese, French). Event intent wins over form/control nouns so
/// "bind the click event to my button" does not route to form layout.
pub fn route_specialist(prompt: &str) -> &'static str {
    let lower = prompt.to_lowercase();

    if contains_any(
        &lower,
        &[
            "event",
            "evento",
            "événement",
            "evenement",
            "イベント",
            "事件",
            "click",
            "clic",
            "clique",
            "クリック",
            "单击",
            "点击",
            "handler",
            "manejador",
            "manipulador",
            "gestionnaire",
            "ハンドラ",
            "处理",
            "bind",
            "wire",
            "wiring",
            "connect",
            "hook",
            "handle",
            "make the",
            "make my",
            "when pressed",
            "when clicked",
            "action",
            "enlazar",
            "vincular",
            "ligar",
            "conectar",
            "conecte",
            "conecta",
            "acionar",
            "acione",
            "quando clicar",
            "ao clicar",
            "acción",
            "acao",
            "ação",
            "associer",
            "connecter",
            "バインド",
            "绑定",
            "onload",
            "onclose",
            "save/update/delete",
            "save",
            "update",
            "delete",
            "navigate",
            "next",
            "previous",
            "first",
            "last",
            "indexed file",
            "indexed",
            "guardar",
            "salvar",
            "actualizar",
            "atualizar",
            "eliminar",
            "excluir",
            "borrar",
            "navegar",
            "siguiente",
            "próximo",
            "proximo",
            "anterior",
        ],
    ) {
        return "EventBinder";
    }

    if contains_any(
        &lower,
        &[
            "form",
            "formulario",
            "formulário",
            "formulaire",
            "フォーム",
            "表单",
            "窗体",
            "ui",
            "screen",
            "pantalla",
            "tela",
            "janela",
            "écran",
            "ecran",
            "画面",
            "屏幕",
            "control",
            "controle",
            "contrôle",
            "コントロール",
            "控件",
            "button",
            "boton",
            "botón",
            "botao",
            "botão",
            "bouton",
            "ボタン",
            "按钮",
            "tabcontrol",
            "tab control",
            "tab1",
            "aba",
            "pestaña",
            "onglet",
            "タブ",
            "选项卡",
            "add",
            "create",
            "insert",
            "place",
            "deploy",
            "añade",
            "agrega",
            "agregue",
            "adicion",
            "adicione",
            "coloca",
            "poner",
            "ponga",
            "ajouter",
            "créer",
            "creer",
            "insérer",
            "inserer",
            "追加",
            "作成",
            "添加",
            "新增",
            "创建",
        ],
    ) {
        return "FormsDesigner";
    }

    "CodeGenerator"
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_crud_navigation_wiring_to_event_binder() {
        assert_eq!(
            route_specialist(
                "wire the Save, Update, Delete, Next and Previous buttons to the indexed file"
            ),
            "EventBinder"
        );
        assert_eq!(
            route_specialist("conecte guardar actualizar eliminar y navegar el archivo indexado"),
            "EventBinder"
        );
        assert_eq!(
            route_specialist("fazer os botões salvar atualizar excluir e navegar funcionarem"),
            "EventBinder"
        );
    }

    #[test]
    fn routes_plain_layout_request_to_forms_designer() {
        assert_eq!(
            route_specialist("add a button to Tab1 of TabControl-1"),
            "FormsDesigner"
        );
    }

    #[test]
    fn named_project_agent_uses_only_its_host_prompt() {
        let prompt = "Grace must end planning with workflow JSON.";
        assert_eq!(compose_system_prompt(prompt, None), prompt);
        assert!(!compose_system_prompt(prompt, None).contains("Do not output JSON"));
    }

    #[test]
    fn builtin_lookup_is_exact() {
        assert!(Specialist::builtin("FormsDesigner").is_some());
        assert!(Specialist::builtin("EventBinder").is_some());
        assert!(Specialist::builtin("CodeGenerator").is_some());
        assert!(Specialist::builtin("Form Designer Agent").is_none());
        assert!(Specialist::builtin("Grace").is_none());
    }

    #[test]
    fn empty_host_prompt_falls_back_to_specialist_preamble() {
        let s = Specialist::new("FormsDesigner");
        let composed = compose_system_prompt("", Some(&s));
        assert_eq!(composed, s.system_prompt);
        let combined = compose_system_prompt("HOST RULES", Some(&s));
        assert!(combined.starts_with("HOST RULES\n\n"));
        assert!(combined.contains("Forms Designer Agent"));
    }
}
