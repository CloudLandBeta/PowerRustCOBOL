//! The deployment kit the complete Web Build writes beside the application (`hosting/deploy/`):
//! an nginx site with the secure-WebSocket proxy, the Dockerfiles, the Amazon ECR push script, the
//! ECS Fargate task and service, the API Gateway WebSocket API, and one `README-<code>.md` per
//! language (en es pt fr jp cn). Every file is plain text produced here; none holds a secret: the
//! account, the region, the domain and the certificate are placeholders the operator fills in.

/// The facts the files are written from.
#[derive(Debug, Clone)]
pub struct Kit<'a> {
    /// The project's name, as the Build knows it.
    pub project: &'a str,
    /// The Content Security Policy and the other headers every response carries.
    pub headers: &'a [(String, String)],
}

/// The six documentation languages: `(file code, name)`.
pub const LANGUAGES: [(&str, &str); 6] = [("en", "English"), ("es", "Español"), ("pt", "Português"), ("fr", "Français"), ("jp", "日本語"), ("cn", "中文")];

/// The name a container image, a task and a service are given: lowercase letters, digits and dashes.
pub fn slug(project: &str) -> String {
    let mut s: String = project.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_owned();
    if s.is_empty() {
        "app".into()
    } else {
        s
    }
}

/// Every file of the kit, `(path under dist/web, text)`.
pub fn files(k: &Kit) -> Vec<(String, String)> {
    let n = slug(k.project);
    let mut out = vec![
        ("hosting/deploy/nginx/site.conf".to_owned(), nginx_site(k, &n)),
        ("hosting/deploy/docker/Dockerfile.backend".to_owned(), dockerfile_backend(&n)),
        ("hosting/deploy/docker/Dockerfile.web".to_owned(), dockerfile_web()),
        ("hosting/deploy/docker/nginx-task.conf".to_owned(), nginx_task(k)),
        ("hosting/deploy/aws/1-push-images.sh".to_owned(), push_script(&n)),
        ("hosting/deploy/aws/2-ecs-task-definition.json".to_owned(), task_definition(&n)),
        ("hosting/deploy/aws/3-create-service.sh".to_owned(), service_script(&n)),
        ("hosting/deploy/aws/4-api-gateway-websocket.sh".to_owned(), apigw_script(&n)),
    ];
    for (code, _) in LANGUAGES {
        out.push((format!("hosting/deploy/README-{code}.md"), readme(code, k.project, &n)));
    }
    out
}

fn add_headers(k: &Kit, indent: &str) -> String {
    let mut s = String::new();
    for (key, v) in k.headers {
        s.push_str(&format!("{indent}add_header {key} \"{}\" always;\n", v.replace('"', "\\\"")));
    }
    s
}

fn nginx_site(k: &Kit, n: &str) -> String {
    format!(
        r#"# {n}: nginx in front of the static bundle and of the Backend (copy to /etc/nginx/conf.d/{n}.conf).
# Replace app.example.com and the two certificate paths. The Backend listens on 127.0.0.1:8443
# (backend/backend.toml, [server] listen) and nginx ends TLS for it: the browser talks wss on 443.
server {{
    listen 80;
    server_name app.example.com;
    return 301 https://$host$request_uri;
}}

server {{
    listen 443 ssl;
    http2 on;
    server_name app.example.com;
    ssl_certificate     /etc/letsencrypt/live/app.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/app.example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;

    root /var/www/{n};
    index index.html;
    types {{ application/wasm wasm; }}
{headers}
    location ~* \.[0-9a-f]{{8}}\.(js|css|wasm|cfrm|json)$ {{ add_header Cache-Control "{cache}"; }}
    location = /index.html {{ add_header Cache-Control "no-cache"; }}

    # The Backend: the page opens wss://app.example.com/ws ([web] backend_url).
    location /ws {{
        proxy_pass http://127.0.0.1:8443;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }}
    location /auth/ {{
        proxy_pass http://127.0.0.1:8443;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto https;
    }}
}}
"#,
        headers = add_headers(k, "    "),
        cache = crate::csp::IMMUTABLE_CACHE,
    )
}

fn nginx_task(k: &Kit) -> String {
    format!(
        r#"# Inside the Fargate task: nginx listens on 8080 behind the load balancer (which ends TLS) and
# reaches the Backend container on the task's own loopback (awsvpc: containers share it).
server {{
    listen 8080;
    root /usr/share/nginx/html;
    index index.html;
    types {{ application/wasm wasm; }}
{headers}
    location ~* \.[0-9a-f]{{8}}\.(js|css|wasm|cfrm|json)$ {{ add_header Cache-Control "{cache}"; }}
    location = /index.html {{ add_header Cache-Control "no-cache"; }}
    location = /healthz {{ return 200 "ok"; add_header Content-Type text/plain; }}
    location /ws {{
        proxy_pass http://127.0.0.1:8443;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
    }}
    location /auth/ {{
        proxy_pass http://127.0.0.1:8443;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto https;
    }}
}}
"#,
        headers = add_headers(k, "    "),
        cache = crate::csp::IMMUTABLE_CACHE,
    )
}

fn dockerfile_backend(n: &str) -> String {
    format!(
        r#"# The Backend image. Build from the project folder, after `rcrun backend build`:
#   docker build -f dist/web/hosting/deploy/docker/Dockerfile.backend -t {n}-backend .
# bin/{n}-backend is the headless Backend binary; backend/backend.toml must say
#   [server] listen = "127.0.0.1:8443"   and   allowed_origins = ["https://app.example.com"]
FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --home /app app
WORKDIR /app
COPY bin/{n}-backend /app/backend
COPY backend/backend.toml backend/exports.toml /app/backend/
COPY data /app/data
USER app
EXPOSE 8443
# Secrets are never baked in: the task passes them as environment variables ("env:NAME" in backend.toml).
CMD ["/app/backend", "--config", "/app/backend/backend.toml"]
"#
    )
}

fn dockerfile_web() -> String {
    r#"# The static site image (nginx). Build from the project folder:
#   docker build -f dist/web/hosting/deploy/docker/Dockerfile.web -t web .
FROM nginx:stable-alpine
COPY dist/web/ /usr/share/nginx/html/
COPY dist/web/hosting/deploy/docker/nginx-task.conf /etc/nginx/conf.d/default.conf
EXPOSE 8080
"#
    .to_owned()
}

fn push_script(n: &str) -> String {
    format!(
        r#"#!/bin/sh
# Builds the two images and pushes them to Amazon ECR. Run from the project folder.
# Needs the AWS CLI signed in (aws configure sso / aws sso login) and Docker. No key is written here.
set -eu
: "${{AWS_ACCOUNT_ID:?set AWS_ACCOUNT_ID}}"
: "${{AWS_REGION:?set AWS_REGION}}"
TAG="${{TAG:-$(date +%Y%m%d%H%M%S)}}"
REGISTRY="$AWS_ACCOUNT_ID.dkr.ecr.$AWS_REGION.amazonaws.com"

for repo in {n}-backend {n}-web; do
  aws ecr describe-repositories --region "$AWS_REGION" --repository-names "$repo" >/dev/null 2>&1 \
    || aws ecr create-repository --region "$AWS_REGION" --repository-name "$repo" --image-scanning-configuration scanOnPush=true
done

aws ecr get-login-password --region "$AWS_REGION" | docker login --username AWS --password-stdin "$REGISTRY"

docker build --platform linux/amd64 -f dist/web/hosting/deploy/docker/Dockerfile.backend -t "$REGISTRY/{n}-backend:$TAG" .
docker build --platform linux/amd64 -f dist/web/hosting/deploy/docker/Dockerfile.web     -t "$REGISTRY/{n}-web:$TAG" .
docker push "$REGISTRY/{n}-backend:$TAG"
docker push "$REGISTRY/{n}-web:$TAG"
echo "Pushed tag $TAG. Next: TAG=$TAG sh 3-create-service.sh"
"#
    )
}

fn task_definition(n: &str) -> String {
    format!(
        r#"{{
  "family": "{n}",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "512",
  "memory": "1024",
  "executionRoleArn": "arn:aws:iam::<AWS_ACCOUNT_ID>:role/ecsTaskExecutionRole",
  "containerDefinitions": [
    {{
      "name": "web",
      "image": "<AWS_ACCOUNT_ID>.dkr.ecr.<AWS_REGION>.amazonaws.com/{n}-web:<TAG>",
      "essential": true,
      "portMappings": [{{ "containerPort": 8080, "protocol": "tcp" }}],
      "dependsOn": [{{ "containerName": "backend", "condition": "START" }}],
      "logConfiguration": {{ "logDriver": "awslogs", "options": {{ "awslogs-group": "/ecs/{n}", "awslogs-region": "<AWS_REGION>", "awslogs-stream-prefix": "web" }} }}
    }},
    {{
      "name": "backend",
      "image": "<AWS_ACCOUNT_ID>.dkr.ecr.<AWS_REGION>.amazonaws.com/{n}-backend:<TAG>",
      "essential": true,
      "secrets": [
        {{ "name": "PRC_COOKIE_KEY", "valueFrom": "arn:aws:secretsmanager:<AWS_REGION>:<AWS_ACCOUNT_ID>:secret:{n}/cookie-key" }}
      ],
      "logConfiguration": {{ "logDriver": "awslogs", "options": {{ "awslogs-group": "/ecs/{n}", "awslogs-region": "<AWS_REGION>", "awslogs-stream-prefix": "backend" }} }}
    }}
  ]
}}
"#
    )
}

fn service_script(n: &str) -> String {
    format!(
        r#"#!/bin/sh
# Registers the task, creates the load balancer target group and the Fargate service.
# Set: AWS_ACCOUNT_ID AWS_REGION TAG CLUSTER VPC_ID SUBNET_IDS (comma list) SECURITY_GROUP_ID TARGET_GROUP_ARN
# The Application Load Balancer (HTTPS 443, an ACM certificate, idle timeout 3600) forwards to TARGET_GROUP_ARN
# (HTTP 8080, health check /healthz); it carries WebSocket upgrades natively.
set -eu
: "${{AWS_ACCOUNT_ID:?}}" "${{AWS_REGION:?}}" "${{TAG:?}}" "${{CLUSTER:?}}" "${{SUBNET_IDS:?}}" "${{SECURITY_GROUP_ID:?}}" "${{TARGET_GROUP_ARN:?}}"
HERE="$(cd "$(dirname "$0")" && pwd)"
sed -e "s/<AWS_ACCOUNT_ID>/$AWS_ACCOUNT_ID/g" -e "s/<AWS_REGION>/$AWS_REGION/g" -e "s/<TAG>/$TAG/g" \
    "$HERE/2-ecs-task-definition.json" > "${{TMPDIR:-/tmp}}/{n}-task.json"
aws logs create-log-group --region "$AWS_REGION" --log-group-name "/ecs/{n}" 2>/dev/null || true
aws ecs create-cluster --region "$AWS_REGION" --cluster-name "$CLUSTER" >/dev/null
aws ecs register-task-definition --region "$AWS_REGION" --cli-input-json "file://${{TMPDIR:-/tmp}}/{n}-task.json" >/dev/null
if aws ecs describe-services --region "$AWS_REGION" --cluster "$CLUSTER" --services {n} --query 'services[?status==`ACTIVE`]' --output text | grep -q .; then
  aws ecs update-service --region "$AWS_REGION" --cluster "$CLUSTER" --service {n} --task-definition {n} --force-new-deployment >/dev/null
else
  aws ecs create-service --region "$AWS_REGION" --cluster "$CLUSTER" --service-name {n} --task-definition {n} \
    --desired-count 2 --launch-type FARGATE \
    --network-configuration "awsvpcConfiguration={{subnets=[$SUBNET_IDS],securityGroups=[$SECURITY_GROUP_ID],assignPublicIp=DISABLED}}" \
    --load-balancers "targetGroupArn=$TARGET_GROUP_ARN,containerName=web,containerPort=8080" >/dev/null
fi
echo "Service {n} is rolling out on cluster $CLUSTER."
"#
    )
}

fn apigw_script(n: &str) -> String {
    format!(
        r#"#!/bin/sh
# OPTIONAL: an API Gateway WebSocket API in front of the Fargate service, through a VPC link to the
# load balancer. Read the Caveats in README first: API Gateway WebSocket APIs deliver each message to the
# integration as a separate HTTP request and accept text frames only; the Backend's sealed channel uses
# binary frames, so the load balancer route (3-create-service.sh) is the supported path.
# Set: AWS_REGION VPC_LINK_ID NLB_LISTENER_URI (http://<internal-nlb-dns>:8080/ws)
set -eu
: "${{AWS_REGION:?}}" "${{VPC_LINK_ID:?}}" "${{NLB_LISTENER_URI:?}}"
API_ID=$(aws apigatewayv2 create-api --region "$AWS_REGION" --name {n}-ws --protocol-type WEBSOCKET \
  --route-selection-expression '$request.body.action' --query ApiId --output text)
INTEGRATION_ID=$(aws apigatewayv2 create-integration --region "$AWS_REGION" --api-id "$API_ID" \
  --integration-type HTTP_PROXY --integration-method POST --integration-uri "$NLB_LISTENER_URI" \
  --connection-type VPC_LINK --connection-id "$VPC_LINK_ID" --query IntegrationId --output text)
for route in '$connect' '$disconnect' '$default'; do
  aws apigatewayv2 create-route --region "$AWS_REGION" --api-id "$API_ID" --route-key "$route" --target "integrations/$INTEGRATION_ID" >/dev/null
done
aws apigatewayv2 create-stage --region "$AWS_REGION" --api-id "$API_ID" --stage-name prod --auto-deploy >/dev/null
echo "wss://$API_ID.execute-api.$AWS_REGION.amazonaws.com/prod"
"#
    )
}

/// One language's sentences, in the order `readme` uses them.
struct Words {
    title: &'static str,
    intro: &'static str,
    contents: &'static str,
    h_before: &'static str,
    before: [&'static str; 4],
    h_nginx: &'static str,
    nginx: &'static str,
    h_docker: &'static str,
    docker: &'static str,
    h_ecr: &'static str,
    ecr: &'static str,
    h_fargate: &'static str,
    fargate: &'static str,
    h_apigw: &'static str,
    apigw: &'static str,
    h_final: &'static str,
    final_: [&'static str; 3],
    h_caveats: &'static str,
    caveats: [&'static str; 3],
}

fn words(code: &str) -> Words {
    match code {
        "es" => Words {
            title: "Guía de despliegue",
            intro: "Este kit lo genera la compilación completa de la Web. Contiene la configuración de nginx, los Dockerfile, los scripts para publicar en Amazon ECR y ejecutar en ECS Fargate, y el script opcional de API Gateway WebSocket. Ningún archivo contiene credenciales: sustituya los marcadores `<…>` y las variables de entorno.",
            contents: "Archivos",
            h_before: "Antes de empezar",
            before: ["Ejecute `rcrun backend build` en la carpeta del proyecto: crea `bin/<nombre>-backend`.", "En `backend/backend.toml` deje `[server] listen = \"127.0.0.1:8443\"` y escriba en `allowed_origins` la dirección pública (`https://app.example.com`).", "En las propiedades del proyecto, Web, ponga `backend_url = \"wss://app.example.com/ws\"` y vuelva a compilar: la página recibe esa dirección.", "Los secretos (`PRC_COOKIE_KEY`, claves del proveedor de identidad) nunca van en la imagen: use `env:NOMBRE` en `backend.toml` y páselos como variables o con AWS Secrets Manager."],
            h_nginx: "Opción A: servidor nginx propio",
            nginx: "Copie `dist/web/` a `/var/www/<proyecto>`, copie `nginx/site.conf` a `/etc/nginx/conf.d/`, cambie el dominio y los certificados (por ejemplo con certbot), inicie `rcrun`/el binario del Backend en el mismo equipo y ejecute `nginx -t && nginx -s reload`. nginx termina TLS: el navegador usa `wss` en el puerto 443 y nginx habla con el Backend por el bucle local.",
            h_docker: "Empaquetar: imágenes Docker",
            docker: "`Dockerfile.backend` empaqueta el binario del Backend; `Dockerfile.web` empaqueta el sitio estático con nginx. Se construyen desde la carpeta del proyecto (ver el encabezado de cada archivo).",
            h_ecr: "Publicar en Amazon ECR",
            ecr: "Inicie sesión con la AWS CLI, defina `AWS_ACCOUNT_ID` y `AWS_REGION` y ejecute `sh dist/web/hosting/deploy/aws/1-push-images.sh`. Crea los repositorios si faltan, construye las dos imágenes y las sube con una etiqueta.",
            h_fargate: "Opción B: ECS Fargate",
            fargate: "Cree antes un Application Load Balancer con HTTPS en 443 (certificado de ACM), un grupo de destino HTTP 8080 con comprobación `/healthz` y el tiempo de inactividad en 3600 s. Defina las variables indicadas en `3-create-service.sh` y ejecútelo: registra la tarea (`2-ecs-task-definition.json`: contenedores `web` y `backend` en la misma tarea, comparten el bucle local) y crea o actualiza el servicio.",
            h_apigw: "API Gateway WebSocket (opcional)",
            apigw: "`4-api-gateway-websocket.sh` crea una API WebSocket con enlace VPC. Lea las advertencias antes de usarla.",
            h_final: "Comprobar",
            final_: ["Abra `https://app.example.com`: la página debe cargar sin la pantalla roja.", "Compruebe en la consola del navegador que la conexión es `wss://` y que no hay errores de CSP.", "Para actualizar: compile de nuevo, ejecute `1-push-images.sh` y `3-create-service.sh` con la nueva etiqueta."],
            h_caveats: "Advertencias",
            caveats: ["API Gateway WebSocket entrega cada mensaje como una solicitud HTTP y solo admite tramas de texto; el canal seguro del Backend usa tramas binarias. La ruta con el balanceador (opción B) es la recomendada.", "Los valores de ejemplo (dominio, CPU, memoria, dos tareas) son un punto de partida; ajústelos a su carga.", "El Backend con varias tareas necesita `idempotency_store = \"server\"` en `[contract]` si repite llamadas tras una caída."],
        },
        "pt" => Words {
            title: "Guia de implantação",
            intro: "Este kit é gerado pela compilação completa da Web. Contém a configuração do nginx, os Dockerfiles, os scripts para publicar no Amazon ECR e executar no ECS Fargate e o script opcional do API Gateway WebSocket. Nenhum arquivo contém credenciais: substitua os marcadores `<…>` e as variáveis de ambiente.",
            contents: "Arquivos",
            h_before: "Antes de começar",
            before: ["Execute `rcrun backend build` na pasta do projeto: ele cria `bin/<nome>-backend`.", "Em `backend/backend.toml` mantenha `[server] listen = \"127.0.0.1:8443\"` e informe em `allowed_origins` o endereço público (`https://app.example.com`).", "Nas propriedades do projeto, Web, defina `backend_url = \"wss://app.example.com/ws\"` e compile de novo: a página recebe esse endereço.", "Os segredos (`PRC_COOKIE_KEY`, chaves do provedor de identidade) nunca vão na imagem: use `env:NOME` no `backend.toml` e passe-os como variáveis ou pelo AWS Secrets Manager."],
            h_nginx: "Opção A: servidor nginx próprio",
            nginx: "Copie `dist/web/` para `/var/www/<projeto>`, copie `nginx/site.conf` para `/etc/nginx/conf.d/`, troque o domínio e os certificados (por exemplo com certbot), inicie o binário do Backend na mesma máquina e execute `nginx -t && nginx -s reload`. O nginx encerra o TLS: o navegador usa `wss` na porta 443 e o nginx fala com o Backend pelo loopback.",
            h_docker: "Empacotar: imagens Docker",
            docker: "`Dockerfile.backend` empacota o binário do Backend; `Dockerfile.web` empacota o site estático com nginx. Construa a partir da pasta do projeto (veja o cabeçalho de cada arquivo).",
            h_ecr: "Publicar no Amazon ECR",
            ecr: "Entre com a AWS CLI, defina `AWS_ACCOUNT_ID` e `AWS_REGION` e execute `sh dist/web/hosting/deploy/aws/1-push-images.sh`. Ele cria os repositórios que faltarem, constrói as duas imagens e as envia com uma tag.",
            h_fargate: "Opção B: ECS Fargate",
            fargate: "Crie antes um Application Load Balancer com HTTPS na 443 (certificado do ACM), um grupo de destino HTTP 8080 com verificação `/healthz` e tempo ocioso de 3600 s. Defina as variáveis indicadas em `3-create-service.sh` e execute-o: ele registra a tarefa (`2-ecs-task-definition.json`: contêineres `web` e `backend` na mesma tarefa, compartilhando o loopback) e cria ou atualiza o serviço.",
            h_apigw: "API Gateway WebSocket (opcional)",
            apigw: "`4-api-gateway-websocket.sh` cria uma API WebSocket com VPC link. Leia os avisos antes de usá-la.",
            h_final: "Verificar",
            final_: ["Abra `https://app.example.com`: a página deve carregar sem a tela vermelha.", "Confirme no console do navegador que a conexão é `wss://` e que não há erros de CSP.", "Para atualizar: compile de novo, execute `1-push-images.sh` e `3-create-service.sh` com a nova tag."],
            h_caveats: "Avisos",
            caveats: ["O API Gateway WebSocket entrega cada mensagem como uma requisição HTTP e aceita apenas quadros de texto; o canal seguro do Backend usa quadros binários. O caminho pelo balanceador (opção B) é o recomendado.", "Os valores de exemplo (domínio, CPU, memória, duas tarefas) são um ponto de partida; ajuste à sua carga.", "Com várias tarefas, o Backend precisa de `idempotency_store = \"server\"` em `[contract]` se repete chamadas após uma queda."],
        },
        "fr" => Words {
            title: "Guide de déploiement",
            intro: "Ce kit est produit par la compilation complète du Web. Il contient la configuration nginx, les Dockerfile, les scripts pour publier sur Amazon ECR et exécuter sur ECS Fargate, et le script optionnel d'API Gateway WebSocket. Aucun fichier ne contient d'identifiants : remplacez les repères `<…>` et les variables d'environnement.",
            contents: "Fichiers",
            h_before: "Avant de commencer",
            before: ["Exécutez `rcrun backend build` dans le dossier du projet : il crée `bin/<nom>-backend`.", "Dans `backend/backend.toml`, gardez `[server] listen = \"127.0.0.1:8443\"` et inscrivez l'adresse publique dans `allowed_origins` (`https://app.example.com`).", "Dans les propriétés du projet, Web, indiquez `backend_url = \"wss://app.example.com/ws\"` puis recompilez : la page reçoit cette adresse.", "Les secrets (`PRC_COOKIE_KEY`, clés du fournisseur d'identité) ne vont jamais dans l'image : utilisez `env:NOM` dans `backend.toml` et fournissez-les en variables ou via AWS Secrets Manager."],
            h_nginx: "Option A : serveur nginx personnel",
            nginx: "Copiez `dist/web/` vers `/var/www/<projet>`, copiez `nginx/site.conf` vers `/etc/nginx/conf.d/`, changez le domaine et les certificats (par exemple avec certbot), lancez le binaire du Backend sur la même machine, puis `nginx -t && nginx -s reload`. nginx termine TLS : le navigateur utilise `wss` sur le port 443 et nginx parle au Backend par la boucle locale.",
            h_docker: "Empaqueter : images Docker",
            docker: "`Dockerfile.backend` empaquette le binaire du Backend ; `Dockerfile.web` empaquette le site statique avec nginx. On les construit depuis le dossier du projet (voir l'en-tête de chaque fichier).",
            h_ecr: "Publier sur Amazon ECR",
            ecr: "Connectez-vous avec l'AWS CLI, définissez `AWS_ACCOUNT_ID` et `AWS_REGION`, puis exécutez `sh dist/web/hosting/deploy/aws/1-push-images.sh`. Il crée les dépôts manquants, construit les deux images et les envoie avec une étiquette.",
            h_fargate: "Option B : ECS Fargate",
            fargate: "Créez d'abord un Application Load Balancer en HTTPS sur 443 (certificat ACM), un groupe cible HTTP 8080 avec le contrôle `/healthz` et un délai d'inactivité de 3600 s. Définissez les variables de `3-create-service.sh` puis exécutez-le : il enregistre la tâche (`2-ecs-task-definition.json` : conteneurs `web` et `backend` dans la même tâche, qui partagent la boucle locale) et crée ou met à jour le service.",
            h_apigw: "API Gateway WebSocket (optionnel)",
            apigw: "`4-api-gateway-websocket.sh` crée une API WebSocket avec un lien VPC. Lisez les mises en garde avant de l'utiliser.",
            h_final: "Vérifier",
            final_: ["Ouvrez `https://app.example.com` : la page doit se charger sans l'écran rouge.", "Vérifiez dans la console du navigateur que la connexion est en `wss://` et sans erreur CSP.", "Pour mettre à jour : recompilez, puis exécutez `1-push-images.sh` et `3-create-service.sh` avec la nouvelle étiquette."],
            h_caveats: "Mises en garde",
            caveats: ["API Gateway WebSocket livre chaque message comme une requête HTTP et n'accepte que des trames texte ; le canal sécurisé du Backend utilise des trames binaires. La voie par l'équilibreur (option B) est celle recommandée.", "Les valeurs d'exemple (domaine, CPU, mémoire, deux tâches) sont un point de départ ; adaptez-les à votre charge.", "Avec plusieurs tâches, le Backend exige `idempotency_store = \"server\"` dans `[contract]` s'il répète des appels après une coupure."],
        },
        "jp" => Words {
            title: "デプロイガイド",
            intro: "このキットは Web の完全ビルドで生成されます。nginx 設定、Dockerfile、Amazon ECR への公開と ECS Fargate での実行用スクリプト、任意の API Gateway WebSocket スクリプトを含みます。認証情報はどのファイルにも含まれません。`<…>` の箇所と環境変数を置き換えてください。",
            contents: "ファイル",
            h_before: "始める前に",
            before: ["プロジェクトのフォルダーで `rcrun backend build` を実行します。`bin/<名前>-backend` が作成されます。", "`backend/backend.toml` では `[server] listen = \"127.0.0.1:8443\"` のままにし、`allowed_origins` に公開アドレス（`https://app.example.com`）を書きます。", "プロジェクトのプロパティの Web で `backend_url = \"wss://app.example.com/ws\"` を設定して再ビルドします。ページはこのアドレスを使います。", "シークレット（`PRC_COOKIE_KEY`、ID プロバイダーのキー）はイメージに入れません。`backend.toml` では `env:名前` を使い、環境変数または AWS Secrets Manager で渡します。"],
            h_nginx: "オプション A：自前の nginx サーバー",
            nginx: "`dist/web/` を `/var/www/<プロジェクト>` にコピーし、`nginx/site.conf` を `/etc/nginx/conf.d/` にコピーします。ドメインと証明書（certbot など）を変更し、同じマシンでバックエンドのバイナリを起動して `nginx -t && nginx -s reload` を実行します。nginx が TLS を終端するため、ブラウザーはポート 443 の `wss` を使い、nginx はループバックでバックエンドに接続します。",
            h_docker: "パッケージ化：Docker イメージ",
            docker: "`Dockerfile.backend` はバックエンドのバイナリを、`Dockerfile.web` は nginx 付きの静的サイトをパッケージ化します。プロジェクトのフォルダーからビルドします（各ファイルの先頭を参照）。",
            h_ecr: "Amazon ECR へ公開",
            ecr: "AWS CLI にサインインし、`AWS_ACCOUNT_ID` と `AWS_REGION` を設定して `sh dist/web/hosting/deploy/aws/1-push-images.sh` を実行します。不足しているリポジトリを作成し、2 つのイメージをビルドしてタグ付きでプッシュします。",
            h_fargate: "オプション B：ECS Fargate",
            fargate: "先に 443 の HTTPS（ACM 証明書）の Application Load Balancer、`/healthz` をヘルスチェックとする HTTP 8080 のターゲットグループ、アイドルタイムアウト 3600 秒を作成します。`3-create-service.sh` に示された変数を設定して実行すると、タスク（`2-ecs-task-definition.json`：同じタスク内の `web` と `backend` コンテナーがループバックを共有）を登録し、サービスを作成または更新します。",
            h_apigw: "API Gateway WebSocket（任意）",
            apigw: "`4-api-gateway-websocket.sh` は VPC リンク付きの WebSocket API を作成します。使用前に注意事項をお読みください。",
            h_final: "確認",
            final_: ["`https://app.example.com` を開き、赤い画面なしでページが読み込まれることを確認します。", "ブラウザーのコンソールで接続が `wss://` であり、CSP エラーがないことを確認します。", "更新するには、再ビルドして新しいタグで `1-push-images.sh` と `3-create-service.sh` を実行します。"],
            h_caveats: "注意事項",
            caveats: ["API Gateway の WebSocket は各メッセージを HTTP リクエストとして渡し、テキストフレームのみ対応します。バックエンドのセキュアチャネルはバイナリフレームを使うため、ロードバランサー経由（オプション B）を推奨します。", "例の値（ドメイン、CPU、メモリ、2 タスク）は出発点です。負荷に合わせて調整してください。", "複数タスクでは、切断後に呼び出しを再実行する場合 `[contract]` に `idempotency_store = \"server\"` が必要です。"],
        },
        "cn" => Words {
            title: "部署指南",
            intro: "此套件由 Web 完整构建生成，包含 nginx 配置、Dockerfile、发布到 Amazon ECR 并在 ECS Fargate 上运行的脚本，以及可选的 API Gateway WebSocket 脚本。所有文件都不含凭据：请替换 `<…>` 占位符和环境变量。",
            contents: "文件",
            h_before: "开始之前",
            before: ["在项目文件夹中运行 `rcrun backend build`，它会生成 `bin/<名称>-backend`。", "在 `backend/backend.toml` 中保持 `[server] listen = \"127.0.0.1:8443\"`，并在 `allowed_origins` 中写入公开地址（`https://app.example.com`）。", "在项目属性的 Web 中设置 `backend_url = \"wss://app.example.com/ws\"` 并重新构建，页面将使用该地址。", "密钥（`PRC_COOKIE_KEY`、身份提供商的密钥）绝不放入镜像：在 `backend.toml` 中使用 `env:名称`，并通过环境变量或 AWS Secrets Manager 提供。"],
            h_nginx: "方案 A：自己的 nginx 服务器",
            nginx: "将 `dist/web/` 复制到 `/var/www/<项目>`，将 `nginx/site.conf` 复制到 `/etc/nginx/conf.d/`，修改域名和证书（例如用 certbot），在同一台机器上启动后端二进制文件，然后运行 `nginx -t && nginx -s reload`。nginx 终止 TLS：浏览器通过 443 端口使用 `wss`，nginx 通过本地回环连接后端。",
            h_docker: "打包：Docker 镜像",
            docker: "`Dockerfile.backend` 打包后端二进制文件；`Dockerfile.web` 打包带 nginx 的静态站点。请在项目文件夹中构建（见每个文件开头的说明）。",
            h_ecr: "发布到 Amazon ECR",
            ecr: "用 AWS CLI 登录，设置 `AWS_ACCOUNT_ID` 和 `AWS_REGION`，然后运行 `sh dist/web/hosting/deploy/aws/1-push-images.sh`。它会创建缺少的仓库，构建两个镜像并带标签推送。",
            h_fargate: "方案 B：ECS Fargate",
            fargate: "先创建 443 端口 HTTPS 的 Application Load Balancer（ACM 证书）、以 `/healthz` 做健康检查的 HTTP 8080 目标组，并将空闲超时设为 3600 秒。设置 `3-create-service.sh` 中列出的变量并运行它：它会注册任务（`2-ecs-task-definition.json`：同一任务中的 `web` 与 `backend` 容器共享本地回环），并创建或更新服务。",
            h_apigw: "API Gateway WebSocket（可选）",
            apigw: "`4-api-gateway-websocket.sh` 创建带 VPC 链接的 WebSocket API。使用前请阅读注意事项。",
            h_final: "验证",
            final_: ["打开 `https://app.example.com`：页面应能加载，不出现红色屏幕。", "在浏览器控制台确认连接为 `wss://`，且没有 CSP 错误。", "更新时：重新构建，并用新标签运行 `1-push-images.sh` 和 `3-create-service.sh`。"],
            h_caveats: "注意事项",
            caveats: ["API Gateway WebSocket 把每条消息作为 HTTP 请求传递，且只支持文本帧；后端的安全通道使用二进制帧。推荐使用经负载均衡器的路径（方案 B）。", "示例值（域名、CPU、内存、两个任务）只是起点，请按负载调整。", "多个任务时，如果后端在断线后会重复调用，需要在 `[contract]` 中设置 `idempotency_store = \"server\"`。"],
        },
        _ => Words {
            title: "Deployment guide",
            intro: "This kit is written by the complete Web Build. It holds the nginx configuration, the Dockerfiles, the scripts that publish to Amazon ECR and run on ECS Fargate, and the optional API Gateway WebSocket script. No file holds a credential: replace the `<…>` markers and the environment variables.",
            contents: "Files",
            h_before: "Before you start",
            before: ["Run `rcrun backend build` in the project folder: it creates `bin/<name>-backend`.", "In `backend/backend.toml` keep `[server] listen = \"127.0.0.1:8443\"` and write the public address in `allowed_origins` (`https://app.example.com`).", "In the project's properties, Web, set `backend_url = \"wss://app.example.com/ws\"` and build again: the page is given that address.", "Secrets (`PRC_COOKIE_KEY`, the identity provider's keys) never go into an image: use `env:NAME` in `backend.toml` and pass them as environment variables or through AWS Secrets Manager."],
            h_nginx: "Option A: your own nginx server",
            nginx: "Copy `dist/web/` to `/var/www/<project>`, copy `nginx/site.conf` to `/etc/nginx/conf.d/`, change the domain and the certificates (for example with certbot), start the Backend binary on the same machine and run `nginx -t && nginx -s reload`. nginx ends TLS: the browser uses `wss` on port 443 and nginx reaches the Backend over loopback.",
            h_docker: "Package: Docker images",
            docker: "`Dockerfile.backend` packages the Backend binary; `Dockerfile.web` packages the static site with nginx. Build them from the project folder (see the header of each file).",
            h_ecr: "Publish to Amazon ECR",
            ecr: "Sign in with the AWS CLI, set `AWS_ACCOUNT_ID` and `AWS_REGION`, and run `sh dist/web/hosting/deploy/aws/1-push-images.sh`. It creates the repositories that are missing, builds the two images and pushes them under one tag.",
            h_fargate: "Option B: ECS Fargate",
            fargate: "First create an Application Load Balancer with HTTPS on 443 (an ACM certificate), an HTTP 8080 target group with the health check `/healthz`, and the idle timeout at 3600 s. Set the variables named in `3-create-service.sh` and run it: it registers the task (`2-ecs-task-definition.json`: the `web` and `backend` containers in one task, sharing its loopback) and creates or updates the service.",
            h_apigw: "API Gateway WebSocket (optional)",
            apigw: "`4-api-gateway-websocket.sh` creates a WebSocket API with a VPC link. Read the Caveats before you use it.",
            h_final: "Check it",
            final_: ["Open `https://app.example.com`: the page must load without the red screen.", "Confirm in the browser console that the connection is `wss://` and that there is no CSP error.", "To update: build again, then run `1-push-images.sh` and `3-create-service.sh` with the new tag."],
            h_caveats: "Caveats",
            caveats: ["An API Gateway WebSocket API hands each message to the integration as an HTTP request and accepts text frames only; the Backend's secure channel uses binary frames. The load balancer route (option B) is the recommended one.", "The sample values (domain, CPU, memory, two tasks) are a starting point; tune them to your load.", "With more than one task the Backend needs `idempotency_store = \"server\"` in `[contract]` if it repeats calls after a drop."],
        },
    }
}

fn readme(code: &str, project: &str, n: &str) -> String {
    let w = words(code);
    let mut s = format!("# {project}: {}\n\n{}\n\n## {}\n\n", w.title, w.intro, w.contents);
    for f in [
        "nginx/site.conf",
        "docker/Dockerfile.backend",
        "docker/Dockerfile.web",
        "docker/nginx-task.conf",
        "aws/1-push-images.sh",
        "aws/2-ecs-task-definition.json",
        "aws/3-create-service.sh",
        "aws/4-api-gateway-websocket.sh",
    ] {
        s.push_str(&format!("- `{f}`\n"));
    }
    s.push_str(&format!("\n## {}\n\n", w.h_before));
    for (i, b) in w.before.iter().enumerate() {
        s.push_str(&format!("{}. {b}\n", i + 1));
    }
    s.push_str(&format!("\n## {}\n\n{}\n\n## {}\n\n{}\n\n```sh\ndocker build -f dist/web/hosting/deploy/docker/Dockerfile.backend -t {n}-backend .\ndocker build -f dist/web/hosting/deploy/docker/Dockerfile.web -t {n}-web .\n```\n\n", w.h_nginx, w.nginx, w.h_docker, w.docker));
    s.push_str(&format!("## {}\n\n{}\n\n```sh\nexport AWS_ACCOUNT_ID=<AWS_ACCOUNT_ID> AWS_REGION=<AWS_REGION>\nsh dist/web/hosting/deploy/aws/1-push-images.sh\n```\n\n", w.h_ecr, w.ecr));
    s.push_str(&format!("## {}\n\n{}\n\n```sh\nexport TAG=<TAG> CLUSTER={n} VPC_ID=<VPC_ID> SUBNET_IDS=<SUBNET_A>,<SUBNET_B> SECURITY_GROUP_ID=<SG_ID> TARGET_GROUP_ARN=<TARGET_GROUP_ARN>\nsh dist/web/hosting/deploy/aws/3-create-service.sh\n```\n\n", w.h_fargate, w.fargate));
    s.push_str(&format!("## {}\n\n{}\n\n## {}\n\n", w.h_apigw, w.apigw, w.h_final));
    for (i, b) in w.final_.iter().enumerate() {
        s.push_str(&format!("{}. {b}\n", i + 1));
    }
    s.push_str(&format!("\n## {}\n\n", w.h_caveats));
    for c in w.caveats {
        s.push_str(&format!("- {c}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit() -> Vec<(String, String)> {
        let h = vec![("X-Content-Type-Options".to_owned(), "nosniff".to_owned())];
        files(&Kit { project: "Power Oficina", headers: &h })
    }

    #[test]
    fn the_kit_has_every_file_and_one_readme_per_language() {
        let f = kit();
        for p in ["nginx/site.conf", "docker/Dockerfile.backend", "docker/Dockerfile.web", "docker/nginx-task.conf", "aws/1-push-images.sh", "aws/2-ecs-task-definition.json", "aws/3-create-service.sh", "aws/4-api-gateway-websocket.sh"] {
            assert!(f.iter().any(|(k, _)| *k == format!("hosting/deploy/{p}")), "{p}");
        }
        for (code, _) in LANGUAGES {
            assert!(f.iter().any(|(k, _)| *k == format!("hosting/deploy/README-{code}.md")), "README-{code}.md");
        }
        assert_eq!(f.len(), 8 + 6);
    }

    #[test]
    fn the_readmes_are_distinct_and_carry_the_same_commands() {
        let f = kit();
        let readmes: Vec<&String> = f.iter().filter(|(k, _)| k.contains("README-")).map(|(_, v)| v).collect();
        for (i, a) in readmes.iter().enumerate() {
            for b in &readmes[i + 1..] {
                assert_ne!(a, b, "a README was not translated");
            }
            assert!(a.contains("1-push-images.sh") && a.contains("3-create-service.sh") && a.contains("rcrun backend build"));
            assert!(a.contains("power-oficina-backend"));
        }
    }

    #[test]
    fn no_file_holds_a_credential() {
        for (p, t) in kit() {
            for needle in ["AKIA", "aws_secret_access_key", "BEGIN PRIVATE KEY", "password="] {
                assert!(!t.contains(needle), "{p} holds {needle}");
            }
        }
    }

    #[test]
    fn the_nginx_site_carries_the_headers_and_proxies_the_websocket() {
        let f = kit();
        let site = &f.iter().find(|(k, _)| k.ends_with("nginx/site.conf")).unwrap().1;
        assert!(site.contains("add_header X-Content-Type-Options \"nosniff\" always;"));
        assert!(site.contains("proxy_set_header Upgrade $http_upgrade;") && site.contains("proxy_pass http://127.0.0.1:8443;"));
        assert!(site.contains("listen 443 ssl;"));
    }

    #[test]
    fn the_slug_is_a_valid_image_name() {
        assert_eq!(slug("Power Oficina"), "power-oficina");
        assert_eq!(slug("__"), "app");
    }
}
