// SPDX-License-Identifier: Apache-2.0

//! Mesh-level behaviour that must survive the Rig migration: multilingual
//! specialist routing and prompt composition (the HTTP wire is owned by Rig
//! now — the hand-rolled orchestrator and its wire tests are deleted).

use cobolt_agents::{compose_system_prompt, route_specialist, Specialist};

#[test]
fn router_recognizes_power_rust_cobol_languages() {
    let form_requests = [
        "Add a button to Tab1 of TabControl-1",
        "Añade un botón a la Tab1 del TabControl-1",
        "Adicione um botão na aba Tab1 do TabControl-1",
        "TabControl-1 の Tab1 にボタンを追加",
        "向 TabControl-1 的 Tab1 添加按钮",
        "Ajouter un bouton a l'onglet Tab1 du TabControl-1",
    ];
    for prompt in form_requests {
        assert_eq!(route_specialist(prompt), "FormsDesigner", "{prompt}");
    }

    let event_requests = [
        "Bind the click event to my button",
        "Vincula el evento clic a mi botón",
        "Ligue o evento clique ao meu botão",
        "ボタンのクリックイベントをバインド",
        "绑定按钮的点击事件",
        "Associer l'evenement clic a mon bouton",
    ];
    for prompt in event_requests {
        assert_eq!(route_specialist(prompt), "EventBinder", "{prompt}");
    }

    let event_wiring_requests = [
        "Wire the Save, Update, Delete, Next and Previous buttons to the indexed file",
        "Conecte guardar actualizar eliminar y navegar el archivo indexado",
        "Fazer os botoes salvar atualizar excluir e navegar funcionarem",
    ];
    for prompt in event_wiring_requests {
        assert_eq!(route_specialist(prompt), "EventBinder", "{prompt}");
    }
}

#[test]
fn router_routes_by_domain() {
    assert_eq!(
        route_specialist("I need a new UI form for login"),
        "FormsDesigner"
    );
    assert_eq!(
        route_specialist("Please bind the click event to my button"),
        "EventBinder"
    );
    assert_eq!(
        route_specialist("Write a function to calculate tax"),
        "CodeGenerator"
    );
}

/// Built-in mesh routes combine both contracts; named project agents use
/// their complete host-supplied prompt without a foreign preamble.
#[test]
fn composition_contract_for_builtins_and_project_agents() {
    let forms = Specialist::builtin("FormsDesigner").expect("built-in");
    let combined = compose_system_prompt("HOST CONTRACT", Some(&forms));
    assert!(combined.starts_with("HOST CONTRACT\n\n"));
    assert!(combined.contains("Forms Designer Agent"));

    assert!(Specialist::builtin("Grace").is_none());
    let host_only = compose_system_prompt("Grace plans workflows.", None);
    assert_eq!(host_only, "Grace plans workflows.");
}
