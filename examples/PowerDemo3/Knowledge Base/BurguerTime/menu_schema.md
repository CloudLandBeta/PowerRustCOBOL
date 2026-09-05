# Esquema do Arquivo Indexado: menu

**Caminho:** `/BurguerTime/menu.cidx`  
**Propósito:** Armazenar itens do menu do sistema BurguerTime.

## Definição de Campos

| Campo | PIC | Chave | Descrição |
| :--- | :--- | :--- | :--- |
| `numero` | `99` | Sim | Identificador único do item |
| `titulo` | `X(50)` | Não | Nome do item |
| `descricao` | `X(100)` | Não | Descrição detalhada |
| `caminho_foto` | `X(255)` | Não | Caminho do arquivo de imagem |
| `preco` | `9(5)V99` | Não | Valor unitário |
| `unidade` | `X(10)` | Não | Unidade de medida |

## Análise de Normalização
- **1NF:** Satisfeito. Atributos atômicos.
- **2NF:** Satisfeito. Dependência total da chave primária simples.
- **3NF:** Satisfeito. Sem dependências transitivas.