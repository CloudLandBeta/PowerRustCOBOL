      *> ───────────────────────────────────────────────────────────
      *>  This code was generated automatically by PowerRustCOBOL RAD.
      *>
      *>  DO NOT MODIFY IT DIRECTLY: it is regenerated the next time
      *>  you interact with the Form Designer, so manual edits are lost.
      *>  Edit the form and its event handlers in the Form Designer
      *>  instead.
      *>
      *>  PowerRustCOBOL may change the structure of this generated code
      *>  at any time — without breaking your code's functionality — for
      *>  reasons such as performance improvements, new observability
      *>  features, and bug fixes.
      *>
      *>  PowerRustCOBOL and its components are distributed under the
      *>  Apache 2.0 License.
      *> ───────────────────────────────────────────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.

       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-BOOL IS "Rust.bool"
           CLASS RUST-CHAR IS "Rust.char"
           CLASS RUST-I8 IS "Rust.i8"
           CLASS RUST-I16 IS "Rust.i16"
           CLASS RUST-I32 IS "Rust.i32"
           CLASS RUST-I64 IS "Rust.i64"
           CLASS RUST-I128 IS "Rust.i128"
           CLASS RUST-ISIZE IS "Rust.isize"
           CLASS RUST-U8 IS "Rust.u8"
           CLASS RUST-U16 IS "Rust.u16"
           CLASS RUST-U32 IS "Rust.u32"
           CLASS RUST-U64 IS "Rust.u64"
           CLASS RUST-U128 IS "Rust.u128"
           CLASS RUST-USIZE IS "Rust.usize"
           CLASS RUST-F32 IS "Rust.f32"
           CLASS RUST-F64 IS "Rust.f64"
           CLASS RUST-STR IS "Rust.str"
           CLASS RUST-UNIT IS "Rust.unit"
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-OSSTRING IS "Rust.OsString"
           CLASS RUST-OSSTR IS "Rust.OsStr"
           CLASS RUST-CSTRING IS "Rust.CString"
           CLASS RUST-CSTR IS "Rust.CStr"
           CLASS RUST-PATH IS "Rust.Path"
           CLASS RUST-PATHBUF IS "Rust.PathBuf"
           CLASS RUST-VEC IS "Rust.Vec"
           CLASS RUST-VECDEQUE IS "Rust.VecDeque"
           CLASS RUST-LINKEDLIST IS "Rust.LinkedList"
           CLASS RUST-HASHMAP IS "Rust.HashMap"
           CLASS RUST-BTREEMAP IS "Rust.BTreeMap"
           CLASS RUST-HASHSET IS "Rust.HashSet"
           CLASS RUST-BTREESET IS "Rust.BTreeSet"
           CLASS RUST-BINARYHEAP IS "Rust.BinaryHeap"
           CLASS RUST-OPTION IS "Rust.Option"
           CLASS RUST-RESULT IS "Rust.Result"
           CLASS RUST-BOX IS "Rust.Box"
           CLASS RUST-RC IS "Rust.Rc"
           CLASS RUST-ARC IS "Rust.Arc"
           CLASS RUST-WEAK IS "Rust.Weak"
           CLASS RUST-CELL IS "Rust.Cell"
           CLASS RUST-REFCELL IS "Rust.RefCell"
           CLASS RUST-MUTEX IS "Rust.Mutex"
           CLASS RUST-RWLOCK IS "Rust.RwLock"
           CLASS RUST-COW IS "Rust.Cow"
           CLASS RUST-DURATION IS "Rust.Duration"
           CLASS RUST-INSTANT IS "Rust.Instant"
           CLASS RUST-SYSTEMTIME IS "Rust.SystemTime"
           CLASS RUST-RANGE IS "Rust.Range".
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT TEMPLATE-FILE ASSIGN TO WS-TEMPLATE-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS.
           SELECT SAVE-FILE ASSIGN TO WS-SAVE-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS-OUT.

       DATA DIVISION.
       FILE SECTION.
       FD TEMPLATE-FILE IS GLOBAL.
       01 TEMPLATE-REC PIC X(400).
       FD SAVE-FILE IS GLOBAL.
       01 SAVE-REC PIC X(400).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'MAIN-FORM'.

      *>── DataGrid Dg-Pets CSV export ──────────────────────────
       01 WS-Dg-Pets-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-Dg-Pets-CSV-STATUS  PIC 9       VALUE 0.

      *>── DataGrid Dg-S3 CSV export ──────────────────────────
       01 WS-Dg-S3-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-Dg-S3-CSV-STATUS  PIC 9       VALUE 0.

      *>── DataGrid Dg-Ec2 CSV export ──────────────────────────
       01 WS-Dg-Ec2-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-Dg-Ec2-CSV-STATUS  PIC 9       VALUE 0.

      *>── Timer: Tmr-Stack ──────────────────────────────────────────
       01 WS-Tmr-Stack-INTERVAL   PIC 9(8) VALUE 3000.
       01 WS-Tmr-Stack-ENABLED    PIC 9    VALUE 0.
       01 WS-Tmr-Stack-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Timer: Tmr-Thumbs ──────────────────────────────────────────
       01 WS-Tmr-Thumbs-INTERVAL   PIC 9(8) VALUE 300.
       01 WS-Tmr-Thumbs-ENABLED    PIC 9    VALUE 0.
       01 WS-Tmr-Thumbs-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Timer: Tmr-Player ──────────────────────────────────────────
       01 WS-Tmr-Player-INTERVAL   PIC 9(8) VALUE 250.
       01 WS-Tmr-Player-ENABLED    PIC 9    VALUE 0.
       01 WS-Tmr-Player-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Timer: Tmr-Ec2 ──────────────────────────────────────────
       01 WS-Tmr-Ec2-INTERVAL   PIC 9(8) VALUE 2000.
       01 WS-Tmr-Ec2-ENABLED    PIC 9    VALUE 0.
       01 WS-Tmr-Ec2-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-REGION-DATA GLOBAL.
           05 FILLER PIC X(14) VALUE "us-east-1".
           05 FILLER PIC X(14) VALUE "us-east-2".
           05 FILLER PIC X(14) VALUE "us-west-2".
           05 FILLER PIC X(14) VALUE "eu-west-1".
           05 FILLER PIC X(14) VALUE "eu-central-1".
           05 FILLER PIC X(14) VALUE "sa-east-1".
       01 WS-REGION-TABLE REDEFINES WS-REGION-DATA GLOBAL.
           05 WS-REGION PIC X(14) OCCURS 6 TIMES.
       01 WS-I            PIC 99          GLOBAL.
       01 WS-STATUS       PIC X(60)       GLOBAL.
       01 WS-OUT          PIC X(4000)     GLOBAL.
       01 WS-MSG          PIC X(300)      GLOBAL.
       01 WS-TEMPLATE-PATH PIC X(120)     GLOBAL VALUE "cloudformation/powercloud.yaml".
       01 WS-SAVE-PATH    PIC X(300)      GLOBAL.
       01 WS-FS           PIC XX          GLOBAL.
       01 WS-FS-OUT       PIC XX          GLOBAL.
       01 WS-LINE         PIC X(400)      GLOBAL.
       01 WS-LINE-LEN     PIC 9(4)        GLOBAL.
       01 WS-TEMPLATE     PIC X(24000)    GLOBAL.
       01 WS-TPOS         PIC 9(5)        GLOBAL.
       01 WS-PARAMS       PIC X(26000)    GLOBAL.
       01 WS-STACK-STATE  PIC X(40)       GLOBAL.
       01 WS-OUTPUTS      PIC X(2000)     GLOBAL.
       01 WS-FN-ROLE      PIC X(300)      GLOBAL.
       01 WS-TICKS        PIC 999         GLOBAL VALUE 0.

       01 WS-PET-COUNT   PIC 999     GLOBAL VALUE 0.
       01 WS-PET-TABLE    GLOBAL.
           05 WS-PET OCCURS 200 TIMES.
               10 WS-PET-ID    PIC X(20).
               10 WS-PET-KEY   PIC X(60).
       01 WS-ROW          PIC 999     GLOBAL.
       01 WS-ROWTXT       PIC X(700)  GLOBAL.
       01 WS-THUMB-NEXT   PIC 999     GLOBAL VALUE 1.
       01 WS-PETS-LOADED  PIC X       GLOBAL VALUE "N".
       01 WS-ID           PIC X(20)   GLOBAL.
       01 WS-PET-NAME     PIC X(60)   GLOBAL.
       01 WS-PET-RACE     PIC X(40)   GLOBAL.
       01 WS-PET-AGE      PIC X(5)    GLOBAL.
       01 WS-PET-PHOTO    PIC X(300)  GLOBAL.
       01 WS-PHOTO-KEY    PIC X(60)   GLOBAL.
       01 WS-LOCAL        PIC X(120)  GLOBAL.
       01 WS-THUMB        PIC X(120)  GLOBAL.
       01 WS-URL          PIC X(2000) GLOBAL.
       01 WS-OK           PIC X(40)   GLOBAL.
       01 WS-W            PIC 9(5)    GLOBAL.
       01 WS-H            PIC 9(5)    GLOBAL.
       01 WS-FMT          PIC X(10)   GLOBAL.
       01 WS-JSON         PIC X(1000) GLOBAL.
       01 WS-ANSWER       PIC X       GLOBAL.
       01 WS-EDIT-ID      PIC X(20)   GLOBAL.
       01 WS-NOW          PIC X(21)   GLOBAL.
       01 WS-NUM          PIC ZZZ9    GLOBAL.

       01 WS-TYPE-DATA GLOBAL.
           05 FILLER PIC X(12) VALUE "t2.micro".
           05 FILLER PIC 99 VALUE 1.
           05 FILLER PIC X(8) VALUE "1 GiB".
           05 FILLER PIC X(12) VALUE "t2.small".
           05 FILLER PIC 99 VALUE 1.
           05 FILLER PIC X(8) VALUE "2 GiB".
           05 FILLER PIC X(12) VALUE "t2.medium".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "4 GiB".
           05 FILLER PIC X(12) VALUE "t3.nano".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "0.5 GiB".
           05 FILLER PIC X(12) VALUE "t3.micro".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "1 GiB".
           05 FILLER PIC X(12) VALUE "t3.small".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "2 GiB".
           05 FILLER PIC X(12) VALUE "t3.medium".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "4 GiB".
           05 FILLER PIC X(12) VALUE "t3.large".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "8 GiB".
           05 FILLER PIC X(12) VALUE "t3.xlarge".
           05 FILLER PIC 99 VALUE 4.
           05 FILLER PIC X(8) VALUE "16 GiB".
           05 FILLER PIC X(12) VALUE "m5.large".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "8 GiB".
           05 FILLER PIC X(12) VALUE "m5.xlarge".
           05 FILLER PIC 99 VALUE 4.
           05 FILLER PIC X(8) VALUE "16 GiB".
           05 FILLER PIC X(12) VALUE "c5.large".
           05 FILLER PIC 99 VALUE 2.
           05 FILLER PIC X(8) VALUE "4 GiB".
       01 WS-TYPE-TABLE REDEFINES WS-TYPE-DATA GLOBAL.
           05 WS-TYPE OCCURS 12 TIMES.
               10 WS-TYPE-NAME PIC X(12).
               10 WS-TYPE-CPU  PIC 99.
               10 WS-TYPE-MEM  PIC X(8).
       01 WS-VOICE-DATA GLOBAL.
           05 FILLER PIC X(10) VALUE "Joanna".
           05 FILLER PIC X(10) VALUE "Camila".
           05 FILLER PIC X(10) VALUE "Lucia".
           05 FILLER PIC X(10) VALUE "Lea".
           05 FILLER PIC X(10) VALUE "Vicki".
           05 FILLER PIC X(10) VALUE "Takumi".
       01 WS-VOICE-TABLE REDEFINES WS-VOICE-DATA GLOBAL.
           05 WS-VOICE PIC X(10) OCCURS 6 TIMES.
       01 WS-N            PIC 999     GLOBAL.
       01 WS-T            PIC 99      GLOBAL.
       01 WS-NL           PIC X       GLOBAL VALUE X"0A".
       01 WS-WORK         USAGE OBJECT GLOBAL.
       01 WS-TARGET       PIC X(10)   GLOBAL.
       01 WS-EC2-ID       PIC X(30)   GLOBAL.
       01 WS-EC2-TYPE     PIC X(12)   GLOBAL.
       01 WS-EC2-STATE    PIC X(12)   GLOBAL.
       01 WS-EC2-POWER    PIC X       GLOBAL.
       01 WS-EC2-CPU      PIC 99      GLOBAL.
       01 WS-CPU-TXT      PIC Z9      GLOBAL.
       01 WS-EC2-MEM      PIC X(8)    GLOBAL.
       01 WS-VOICE-IX     PIC 9       GLOBAL.
       01 WS-AUD-STATE    PIC X(10)   GLOBAL.
       01 WS-POS-MS       PIC 9(9)    GLOBAL.
       01 WS-DUR-MS       PIC 9(9)    GLOBAL.
       01 WS-SEC          PIC 9(6)    GLOBAL.
       01 WS-DUR-SEC      PIC 9(6)    GLOBAL.
       01 WS-MIN          PIC 99      GLOBAL.
       01 WS-SECS         PIC 99      GLOBAL.
       01 WS-DMIN         PIC 99      GLOBAL.
       01 WS-DSECS        PIC 99      GLOBAL.
       01 WS-PCT          PIC 999     GLOBAL.
       01 WS-REK-FILE     PIC X(300)  GLOBAL.
       01 WS-PIC-W        PIC 9(5)    GLOBAL.
       01 WS-PIC-H        PIC 9(5)    GLOBAL.
       01 WS-SCALE        PIC 9(3)V9(6) GLOBAL.
       01 WS-SCALE-H      PIC 9(3)V9(6) GLOBAL.
       01 WS-DISP-W       PIC 9(5)V99 GLOBAL.
       01 WS-DISP-H       PIC 9(5)V99 GLOBAL.
       01 WS-OFF-X        PIC 9(5)V99 GLOBAL.
       01 WS-OFF-Y        PIC 9(5)V99 GLOBAL.
       01 WS-BX           PIC 9(5)    GLOBAL.
       01 WS-BY           PIC 9(5)    GLOBAL.
       01 WS-BW           PIC 9(5)    GLOBAL.
       01 WS-BH           PIC 9(5)    GLOBAL.
       01 WS-DETAILS      PIC X(1500) GLOBAL.
       01 WS-DPOS         PIC 9(4)    GLOBAL.
       *>   The interface language (spec 092, amendment A1): WS-LANG-IX is the column of the
       *>   text tables - 1 en, 2 pt, 3 es, 4 fr, 5 jp, 6 cn - and CMB-LANGUAGE sets it.

       01 WS-LANG-IX     PIC 9       GLOBAL VALUE 1.
       01 WS-TX-I        PIC 999     GLOBAL.
       01 WS-TEXT-ROWS   PIC 999     GLOBAL VALUE 110.
       01 WS-WORKING-TEXT PIC X(300) GLOBAL.
       01 WS-MATCH       PIC X(120)  GLOBAL.
       01 WS-MATCH-VAL   PIC X(30)   GLOBAL.
       01 WS-REK-LIST    PIC X       GLOBAL VALUE "N".
       *>   What each status line says: the row of the message in PC-TEXT, and the two
       *>   values that went into its &1 and &2, so a change of language writes it again.

       01 WS-CONN-MSG PIC 999     GLOBAL.
       01 WS-CONN-A1   PIC X(500) GLOBAL.
       01 WS-CONN-A2   PIC X(100) GLOBAL.
       01 WS-STK-MSG  PIC 999     GLOBAL.
       01 WS-STK-A1    PIC X(500) GLOBAL.
       01 WS-STK-A2    PIC X(100) GLOBAL.
       01 WS-DDB-MSG  PIC 999     GLOBAL.
       01 WS-DDB-A1    PIC X(500) GLOBAL.
       01 WS-DDB-A2    PIC X(100) GLOBAL.
       01 WS-S3-MSG   PIC 999     GLOBAL.
       01 WS-S3-A1     PIC X(500) GLOBAL.
       01 WS-S3-A2     PIC X(100) GLOBAL.
       01 WS-LAM-MSG  PIC 999     GLOBAL.
       01 WS-LAM-A1    PIC X(500) GLOBAL.
       01 WS-LAM-A2    PIC X(100) GLOBAL.
       01 WS-POL-MSG  PIC 999     GLOBAL.
       01 WS-POL-A1    PIC X(500) GLOBAL.
       01 WS-POL-A2    PIC X(100) GLOBAL.
       01 WS-EC2-MSG  PIC 999     GLOBAL.
       01 WS-EC2-A1    PIC X(500) GLOBAL.
       01 WS-EC2-A2    PIC X(100) GLOBAL.
       01 WS-REK-MSG  PIC 999     GLOBAL.
       01 WS-REK-A1    PIC X(500) GLOBAL.
       01 WS-REK-A2    PIC X(100) GLOBAL.
       *>   The texts: one row per text, one column per language - en, pt, es, fr, jp, cn.
       *>   English is what the form was designed with; the other five are translations.

       01 PC-TEXT-DATA GLOBAL.
           *>   TAGLINE
           05 FILLER PIC X(120) VALUE "One AWS service on every tab, written in RustCOBOL".
           05 FILLER PIC X(120) VALUE "Um serviço da AWS em cada aba, escrito em RustCOBOL".
           05 FILLER PIC X(120) VALUE "Un servicio de AWS en cada pestaña, escrito en RustCOBOL".
           05 FILLER PIC X(120) VALUE "Un service AWS par onglet, écrit en RustCOBOL".
           05 FILLER PIC X(120) VALUE "タブごとに 1 つの AWS サービス、RustCOBOL で記述".
           05 FILLER PIC X(120) VALUE "每个选项卡一个 AWS 服务，用 RustCOBOL 编写".
           *>   LANG-TIP
           05 FILLER PIC X(120) VALUE "Language".
           05 FILLER PIC X(120) VALUE "Idioma".
           05 FILLER PIC X(120) VALUE "Idioma".
           05 FILLER PIC X(120) VALUE "Langue".
           05 FILLER PIC X(120) VALUE "言語".
           05 FILLER PIC X(120) VALUE "语言".
           *>   TAB-SETTINGS
           05 FILLER PIC X(120) VALUE "Settings".
           05 FILLER PIC X(120) VALUE "Configurações".
           05 FILLER PIC X(120) VALUE "Configuración".
           05 FILLER PIC X(120) VALUE "Paramètres".
           05 FILLER PIC X(120) VALUE "設定".
           05 FILLER PIC X(120) VALUE "设置".
           *>   SET-HEAD
           05 FILLER PIC X(120) VALUE "Cloud access".
           05 FILLER PIC X(120) VALUE "Acesso à nuvem".
           05 FILLER PIC X(120) VALUE "Acceso a la nube".
           05 FILLER PIC X(120) VALUE "Accès au cloud".
           05 FILLER PIC X(120) VALUE "クラウドへのアクセス".
           05 FILLER PIC X(120) VALUE "云访问".
           *>   REGION
           05 FILLER PIC X(120) VALUE "Region".
           05 FILLER PIC X(120) VALUE "Região".
           05 FILLER PIC X(120) VALUE "Región".
           05 FILLER PIC X(120) VALUE "Région".
           05 FILLER PIC X(120) VALUE "リージョン".
           05 FILLER PIC X(120) VALUE "区域".
           *>   KEY-ID
           05 FILLER PIC X(120) VALUE "Access key id".
           05 FILLER PIC X(120) VALUE "ID da chave de acesso".
           05 FILLER PIC X(120) VALUE "ID de clave de acceso".
           05 FILLER PIC X(120) VALUE "ID de clé d'accès".
           05 FILLER PIC X(120) VALUE "アクセスキー ID".
           05 FILLER PIC X(120) VALUE "访问密钥 ID".
           *>   TOKEN
           05 FILLER PIC X(120) VALUE "Session token (optional)".
           05 FILLER PIC X(120) VALUE "Token de sessão (opcional)".
           05 FILLER PIC X(120) VALUE "Token de sesión (opcional)".
           05 FILLER PIC X(120) VALUE "Jeton de session (facultatif)".
           05 FILLER PIC X(120) VALUE "セッショントークン（任意）".
           05 FILLER PIC X(120) VALUE "会话令牌（可选）".
           *>   TOKEN-HINT
           05 FILLER PIC X(120) VALUE "only for temporary keys".
           05 FILLER PIC X(120) VALUE "somente para chaves temporárias".
           05 FILLER PIC X(120) VALUE "solo para claves temporales".
           05 FILLER PIC X(120) VALUE "uniquement pour les clés temporaires".
           05 FILLER PIC X(120) VALUE "一時的なキーの場合のみ".
           05 FILLER PIC X(120) VALUE "仅用于临时密钥".
           *>   SECRET
           05 FILLER PIC X(120) VALUE "Secret access key".
           05 FILLER PIC X(120) VALUE "Chave de acesso secreta".
           05 FILLER PIC X(120) VALUE "Clave de acceso secreta".
           05 FILLER PIC X(120) VALUE "Clé d'accès secrète".
           05 FILLER PIC X(120) VALUE "シークレットアクセスキー".
           05 FILLER PIC X(120) VALUE "秘密访问密钥".
           *>   CONNECT
           05 FILLER PIC X(120) VALUE "Connect".
           05 FILLER PIC X(120) VALUE "Conectar".
           05 FILLER PIC X(120) VALUE "Conectar".
           05 FILLER PIC X(120) VALUE "Se connecter".
           05 FILLER PIC X(120) VALUE "接続".
           05 FILLER PIC X(120) VALUE "连接".
           *>   STACK-NAME
           05 FILLER PIC X(120) VALUE "Stack name".
           05 FILLER PIC X(120) VALUE "Nome da pilha".
           05 FILLER PIC X(120) VALUE "Nombre de la pila".
           05 FILLER PIC X(120) VALUE "Nom de la pile".
           05 FILLER PIC X(120) VALUE "スタック名".
           05 FILLER PIC X(120) VALUE "堆栈名称".
           *>   BUCKET
           05 FILLER PIC X(120) VALUE "Photo bucket".
           05 FILLER PIC X(120) VALUE "Bucket das fotos".
           05 FILLER PIC X(120) VALUE "Bucket de fotos".
           05 FILLER PIC X(120) VALUE "Bucket des photos".
           05 FILLER PIC X(120) VALUE "写真バケット".
           05 FILLER PIC X(120) VALUE "照片存储桶".
           *>   BUCKET-HINT
           05 FILLER PIC X(120) VALUE "filled in by the stack".
           05 FILLER PIC X(120) VALUE "preenchido pela pilha".
           05 FILLER PIC X(120) VALUE "lo rellena la pila".
           05 FILLER PIC X(120) VALUE "rempli par la pile".
           05 FILLER PIC X(120) VALUE "スタックが入力します".
           05 FILLER PIC X(120) VALUE "由堆栈填写".
           *>   FUNCTION
           05 FILLER PIC X(120) VALUE "Function".
           05 FILLER PIC X(120) VALUE "Função".
           05 FILLER PIC X(120) VALUE "Función".
           05 FILLER PIC X(120) VALUE "Fonction".
           05 FILLER PIC X(120) VALUE "関数".
           05 FILLER PIC X(120) VALUE "函数".
           *>   DEPLOY
           05 FILLER PIC X(120) VALUE "Deploy CloudFormation".
           05 FILLER PIC X(120) VALUE "Implantar CloudFormation".
           05 FILLER PIC X(120) VALUE "Implementar CloudFormation".
           05 FILLER PIC X(120) VALUE "Déployer CloudFormation".
           05 FILLER PIC X(120) VALUE "CloudFormation をデプロイ".
           05 FILLER PIC X(120) VALUE "部署 CloudFormation".
           *>   SAVE-TEMPLATE
           05 FILLER PIC X(120) VALUE "Save template…".
           05 FILLER PIC X(120) VALUE "Salvar modelo…".
           05 FILLER PIC X(120) VALUE "Guardar plantilla…".
           05 FILLER PIC X(120) VALUE "Enregistrer le modèle…".
           05 FILLER PIC X(120) VALUE "テンプレートを保存…".
           05 FILLER PIC X(120) VALUE "保存模板…".
           *>   LINK-TEMPLATE
           05 FILLER PIC X(120) VALUE "Download the CloudFormation template".
           05 FILLER PIC X(120) VALUE "Baixar o modelo do CloudFormation".
           05 FILLER PIC X(120) VALUE "Descargar la plantilla de CloudFormation".
           05 FILLER PIC X(120) VALUE "Télécharger le modèle CloudFormation".
           05 FILLER PIC X(120) VALUE "CloudFormation テンプレートをダウンロード".
           05 FILLER PIC X(120) VALUE "下载 CloudFormation 模板".
           *>   SAVE-DIALOG
           05 FILLER PIC X(120) VALUE "Save the CloudFormation template".
           05 FILLER PIC X(120) VALUE "Salvar o modelo do CloudFormation".
           05 FILLER PIC X(120) VALUE "Guardar la plantilla de CloudFormation".
           05 FILLER PIC X(120) VALUE "Enregistrer le modèle CloudFormation".
           05 FILLER PIC X(120) VALUE "CloudFormation テンプレートを保存".
           05 FILLER PIC X(120) VALUE "保存 CloudFormation 模板".
           *>   DDB-HEAD
           05 FILLER PIC X(120) VALUE "Pets".
           05 FILLER PIC X(120) VALUE "Animais".
           05 FILLER PIC X(120) VALUE "Mascotas".
           05 FILLER PIC X(120) VALUE "Animaux".
           05 FILLER PIC X(120) VALUE "ペット".
           05 FILLER PIC X(120) VALUE "宠物".
           *>   CREATE-TABLE
           05 FILLER PIC X(120) VALUE "Create table".
           05 FILLER PIC X(120) VALUE "Criar tabela".
           05 FILLER PIC X(120) VALUE "Crear tabla".
           05 FILLER PIC X(120) VALUE "Créer la table".
           05 FILLER PIC X(120) VALUE "テーブルを作成".
           05 FILLER PIC X(120) VALUE "创建表".
           *>   NEW-PET
           05 FILLER PIC X(120) VALUE "New pet".
           05 FILLER PIC X(120) VALUE "Novo animal".
           05 FILLER PIC X(120) VALUE "Nueva mascota".
           05 FILLER PIC X(120) VALUE "Nouvel animal".
           05 FILLER PIC X(120) VALUE "新しいペット".
           05 FILLER PIC X(120) VALUE "新建宠物".
           *>   RELOAD
           05 FILLER PIC X(120) VALUE "Reload".
           05 FILLER PIC X(120) VALUE "Recarregar".
           05 FILLER PIC X(120) VALUE "Recargar".
           05 FILLER PIC X(120) VALUE "Actualiser".
           05 FILLER PIC X(120) VALUE "再読み込み".
           05 FILLER PIC X(120) VALUE "重新加载".
           *>   CLEAR-FILTERS
           05 FILLER PIC X(120) VALUE "Clear filters".
           05 FILLER PIC X(120) VALUE "Limpar filtros".
           05 FILLER PIC X(120) VALUE "Borrar filtros".
           05 FILLER PIC X(120) VALUE "Effacer les filtres".
           05 FILLER PIC X(120) VALUE "フィルターをクリア".
           05 FILLER PIC X(120) VALUE "清除筛选".
           *>   COL-ID
           05 FILLER PIC X(120) VALUE "ID".
           05 FILLER PIC X(120) VALUE "ID".
           05 FILLER PIC X(120) VALUE "ID".
           05 FILLER PIC X(120) VALUE "ID".
           05 FILLER PIC X(120) VALUE "ID".
           05 FILLER PIC X(120) VALUE "ID".
           *>   COL-PHOTO
           05 FILLER PIC X(120) VALUE "Photo".
           05 FILLER PIC X(120) VALUE "Foto".
           05 FILLER PIC X(120) VALUE "Foto".
           05 FILLER PIC X(120) VALUE "Photo".
           05 FILLER PIC X(120) VALUE "写真".
           05 FILLER PIC X(120) VALUE "照片".
           *>   COL-NAME
           05 FILLER PIC X(120) VALUE "Name".
           05 FILLER PIC X(120) VALUE "Nome".
           05 FILLER PIC X(120) VALUE "Nombre".
           05 FILLER PIC X(120) VALUE "Nom".
           05 FILLER PIC X(120) VALUE "名前".
           05 FILLER PIC X(120) VALUE "名称".
           *>   COL-RACE
           05 FILLER PIC X(120) VALUE "Race".
           05 FILLER PIC X(120) VALUE "Raça".
           05 FILLER PIC X(120) VALUE "Raza".
           05 FILLER PIC X(120) VALUE "Race".
           05 FILLER PIC X(120) VALUE "品種".
           05 FILLER PIC X(120) VALUE "品种".
           *>   COL-AGE
           05 FILLER PIC X(120) VALUE "Age".
           05 FILLER PIC X(120) VALUE "Idade".
           05 FILLER PIC X(120) VALUE "Edad".
           05 FILLER PIC X(120) VALUE "Âge".
           05 FILLER PIC X(120) VALUE "年齢".
           05 FILLER PIC X(120) VALUE "年龄".
           *>   COL-VIEW
           05 FILLER PIC X(120) VALUE "View".
           05 FILLER PIC X(120) VALUE "Ver".
           05 FILLER PIC X(120) VALUE "Ver".
           05 FILLER PIC X(120) VALUE "Voir".
           05 FILLER PIC X(120) VALUE "表示".
           05 FILLER PIC X(120) VALUE "查看".
           *>   COL-EDIT
           05 FILLER PIC X(120) VALUE "Edit".
           05 FILLER PIC X(120) VALUE "Editar".
           05 FILLER PIC X(120) VALUE "Editar".
           05 FILLER PIC X(120) VALUE "Modifier".
           05 FILLER PIC X(120) VALUE "編集".
           05 FILLER PIC X(120) VALUE "编辑".
           *>   COL-DELETE
           05 FILLER PIC X(120) VALUE "Delete".
           05 FILLER PIC X(120) VALUE "Excluir".
           05 FILLER PIC X(120) VALUE "Eliminar".
           05 FILLER PIC X(120) VALUE "Supprimer".
           05 FILLER PIC X(120) VALUE "削除".
           05 FILLER PIC X(120) VALUE "删除".
           *>   S3-HEAD
           05 FILLER PIC X(120) VALUE "Buckets of the account".
           05 FILLER PIC X(120) VALUE "Buckets da conta".
           05 FILLER PIC X(120) VALUE "Buckets de la cuenta".
           05 FILLER PIC X(120) VALUE "Buckets du compte".
           05 FILLER PIC X(120) VALUE "アカウントのバケット".
           05 FILLER PIC X(120) VALUE "账户的存储桶".
           *>   COL-KEY
           05 FILLER PIC X(120) VALUE "Key".
           05 FILLER PIC X(120) VALUE "Chave".
           05 FILLER PIC X(120) VALUE "Clave".
           05 FILLER PIC X(120) VALUE "Clé".
           05 FILLER PIC X(120) VALUE "キー".
           05 FILLER PIC X(120) VALUE "键".
           *>   COL-SIZE
           05 FILLER PIC X(120) VALUE "Size (bytes)".
           05 FILLER PIC X(120) VALUE "Tamanho (bytes)".
           05 FILLER PIC X(120) VALUE "Tamaño (bytes)".
           05 FILLER PIC X(120) VALUE "Taille (octets)".
           05 FILLER PIC X(120) VALUE "サイズ（バイト）".
           05 FILLER PIC X(120) VALUE "大小（字节）".
           *>   COL-MODIFIED
           05 FILLER PIC X(120) VALUE "Last modified".
           05 FILLER PIC X(120) VALUE "Última modificação".
           05 FILLER PIC X(120) VALUE "Última modificación".
           05 FILLER PIC X(120) VALUE "Dernière modification".
           05 FILLER PIC X(120) VALUE "最終更新".
           05 FILLER PIC X(120) VALUE "最后修改".
           *>   LAM-HEAD
           05 FILLER PIC X(120) VALUE "Temporary links, made by a Lambda function".
           05 FILLER PIC X(120) VALUE "Links temporários, gerados por uma função Lambda".
           05 FILLER PIC X(120) VALUE "Enlaces temporales, generados por una función Lambda".
           05 FILLER PIC X(120) VALUE "Liens temporaires, créés par une fonction Lambda".
           05 FILLER PIC X(120) VALUE "Lambda 関数が作る一時リンク".
           05 FILLER PIC X(120) VALUE "由 Lambda 函数生成的临时链接".
           *>   LAM-SRC
           05 FILLER PIC X(120) VALUE "The function, as the stack deploys it".
           05 FILLER PIC X(120) VALUE "A função, como a pilha a implanta".
           05 FILLER PIC X(120) VALUE "La función, tal como la despliega la pila".
           05 FILLER PIC X(120) VALUE "La fonction, telle que la pile la déploie".
           05 FILLER PIC X(120) VALUE "スタックがデプロイする関数".
           05 FILLER PIC X(120) VALUE "堆栈部署的函数".
           *>   LAM-KEY
           05 FILLER PIC X(120) VALUE "Object key".
           05 FILLER PIC X(120) VALUE "Chave do objeto".
           05 FILLER PIC X(120) VALUE "Clave del objeto".
           05 FILLER PIC X(120) VALUE "Clé de l'objet".
           05 FILLER PIC X(120) VALUE "オブジェクトキー".
           05 FILLER PIC X(120) VALUE "对象键".
           *>   LAM-TRY
           05 FILLER PIC X(120) VALUE "Try it".
           05 FILLER PIC X(120) VALUE "Experimentar".
           05 FILLER PIC X(120) VALUE "Probar".
           05 FILLER PIC X(120) VALUE "Essayer".
           05 FILLER PIC X(120) VALUE "試す".
           05 FILLER PIC X(120) VALUE "试一试".
           *>   POLLY-HEAD
           05 FILLER PIC X(120) VALUE "Text to speech".
           05 FILLER PIC X(120) VALUE "Texto para fala".
           05 FILLER PIC X(120) VALUE "Texto a voz".
           05 FILLER PIC X(120) VALUE "Synthèse vocale".
           05 FILLER PIC X(120) VALUE "音声合成".
           05 FILLER PIC X(120) VALUE "文字转语音".
           *>   POLLY-LANG
           05 FILLER PIC X(120) VALUE "Language and voice".
           05 FILLER PIC X(120) VALUE "Idioma e voz".
           05 FILLER PIC X(120) VALUE "Idioma y voz".
           05 FILLER PIC X(120) VALUE "Langue et voix".
           05 FILLER PIC X(120) VALUE "言語と音声".
           05 FILLER PIC X(120) VALUE "语言和语音".
           *>   VOICE-1
           05 FILLER PIC X(120) VALUE "English (Joanna)".
           05 FILLER PIC X(120) VALUE "Inglês (Joanna)".
           05 FILLER PIC X(120) VALUE "Inglés (Joanna)".
           05 FILLER PIC X(120) VALUE "Anglais (Joanna)".
           05 FILLER PIC X(120) VALUE "英語（Joanna）".
           05 FILLER PIC X(120) VALUE "英语（Joanna）".
           *>   VOICE-2
           05 FILLER PIC X(120) VALUE "Portuguese, Brazil (Camila)".
           05 FILLER PIC X(120) VALUE "Português, Brasil (Camila)".
           05 FILLER PIC X(120) VALUE "Portugués, Brasil (Camila)".
           05 FILLER PIC X(120) VALUE "Portugais, Brésil (Camila)".
           05 FILLER PIC X(120) VALUE "ポルトガル語・ブラジル（Camila）".
           05 FILLER PIC X(120) VALUE "葡萄牙语，巴西（Camila）".
           *>   VOICE-3
           05 FILLER PIC X(120) VALUE "Spanish, Spain (Lucia)".
           05 FILLER PIC X(120) VALUE "Espanhol, Espanha (Lucia)".
           05 FILLER PIC X(120) VALUE "Español, España (Lucia)".
           05 FILLER PIC X(120) VALUE "Espagnol, Espagne (Lucia)".
           05 FILLER PIC X(120) VALUE "スペイン語・スペイン（Lucia）".
           05 FILLER PIC X(120) VALUE "西班牙语，西班牙（Lucia）".
           *>   VOICE-4
           05 FILLER PIC X(120) VALUE "French (Lea)".
           05 FILLER PIC X(120) VALUE "Francês (Lea)".
           05 FILLER PIC X(120) VALUE "Francés (Lea)".
           05 FILLER PIC X(120) VALUE "Français (Lea)".
           05 FILLER PIC X(120) VALUE "フランス語（Lea）".
           05 FILLER PIC X(120) VALUE "法语（Lea）".
           *>   VOICE-5
           05 FILLER PIC X(120) VALUE "German (Vicki)".
           05 FILLER PIC X(120) VALUE "Alemão (Vicki)".
           05 FILLER PIC X(120) VALUE "Alemán (Vicki)".
           05 FILLER PIC X(120) VALUE "Allemand (Vicki)".
           05 FILLER PIC X(120) VALUE "ドイツ語（Vicki）".
           05 FILLER PIC X(120) VALUE "德语（Vicki）".
           *>   VOICE-6
           05 FILLER PIC X(120) VALUE "Japanese (Takumi)".
           05 FILLER PIC X(120) VALUE "Japonês (Takumi)".
           05 FILLER PIC X(120) VALUE "Japonés (Takumi)".
           05 FILLER PIC X(120) VALUE "Japonais (Takumi)".
           05 FILLER PIC X(120) VALUE "日本語（Takumi）".
           05 FILLER PIC X(120) VALUE "日语（Takumi）".
           *>   SPEAK
           05 FILLER PIC X(120) VALUE "Speak".
           05 FILLER PIC X(120) VALUE "Falar".
           05 FILLER PIC X(120) VALUE "Hablar".
           05 FILLER PIC X(120) VALUE "Parler".
           05 FILLER PIC X(120) VALUE "読み上げ".
           05 FILLER PIC X(120) VALUE "朗读".
           *>   EC2-HEAD
           05 FILLER PIC X(120) VALUE "Servers".
           05 FILLER PIC X(120) VALUE "Servidores".
           05 FILLER PIC X(120) VALUE "Servidores".
           05 FILLER PIC X(120) VALUE "Serveurs".
           05 FILLER PIC X(120) VALUE "サーバー".
           05 FILLER PIC X(120) VALUE "服务器".
           *>   COL-INSTANCE
           05 FILLER PIC X(120) VALUE "Instance ID".
           05 FILLER PIC X(120) VALUE "ID da instância".
           05 FILLER PIC X(120) VALUE "ID de instancia".
           05 FILLER PIC X(120) VALUE "ID d'instance".
           05 FILLER PIC X(120) VALUE "インスタンス ID".
           05 FILLER PIC X(120) VALUE "实例 ID".
           *>   COL-CLASS
           05 FILLER PIC X(120) VALUE "Class".
           05 FILLER PIC X(120) VALUE "Classe".
           05 FILLER PIC X(120) VALUE "Clase".
           05 FILLER PIC X(120) VALUE "Classe".
           05 FILLER PIC X(120) VALUE "クラス".
           05 FILLER PIC X(120) VALUE "类型".
           *>   COL-VCPU
           05 FILLER PIC X(120) VALUE "vCPU".
           05 FILLER PIC X(120) VALUE "vCPU".
           05 FILLER PIC X(120) VALUE "vCPU".
           05 FILLER PIC X(120) VALUE "vCPU".
           05 FILLER PIC X(120) VALUE "vCPU".
           05 FILLER PIC X(120) VALUE "vCPU".
           *>   COL-MEMORY
           05 FILLER PIC X(120) VALUE "Memory".
           05 FILLER PIC X(120) VALUE "Memória".
           05 FILLER PIC X(120) VALUE "Memoria".
           05 FILLER PIC X(120) VALUE "Mémoire".
           05 FILLER PIC X(120) VALUE "メモリ".
           05 FILLER PIC X(120) VALUE "内存".
           *>   COL-STATE
           05 FILLER PIC X(120) VALUE "State".
           05 FILLER PIC X(120) VALUE "Estado".
           05 FILLER PIC X(120) VALUE "Estado".
           05 FILLER PIC X(120) VALUE "État".
           05 FILLER PIC X(120) VALUE "状態".
           05 FILLER PIC X(120) VALUE "状态".
           *>   COL-POWER
           05 FILLER PIC X(120) VALUE "On / off".
           05 FILLER PIC X(120) VALUE "Ligado / desligado".
           05 FILLER PIC X(120) VALUE "Encendido / apagado".
           05 FILLER PIC X(120) VALUE "Marche / arrêt".
           05 FILLER PIC X(120) VALUE "オン / オフ".
           05 FILLER PIC X(120) VALUE "开 / 关".
           *>   WAIT-RUNNING
           05 FILLER PIC X(120) VALUE "Waiting for &1 to be running…".
           05 FILLER PIC X(120) VALUE "Aguardando &1 entrar em execução…".
           05 FILLER PIC X(120) VALUE "Esperando a que &1 esté en ejecución…".
           05 FILLER PIC X(120) VALUE "En attente du démarrage de &1…".
           05 FILLER PIC X(120) VALUE "&1 が実行中になるのを待っています…".
           05 FILLER PIC X(120) VALUE "正在等待 &1 运行…".
           *>   WAIT-STOPPED
           05 FILLER PIC X(120) VALUE "Waiting for &1 to be stopped…".
           05 FILLER PIC X(120) VALUE "Aguardando &1 ser parado…".
           05 FILLER PIC X(120) VALUE "Esperando a que &1 esté detenido…".
           05 FILLER PIC X(120) VALUE "En attente de l'arrêt de &1…".
           05 FILLER PIC X(120) VALUE "&1 が停止するのを待っています…".
           05 FILLER PIC X(120) VALUE "正在等待 &1 停止…".
           *>   REK-HEAD
           05 FILLER PIC X(120) VALUE "Who is in the picture?".
           05 FILLER PIC X(120) VALUE "Quem está na foto?".
           05 FILLER PIC X(120) VALUE "¿Quién aparece en la foto?".
           05 FILLER PIC X(120) VALUE "Qui est sur la photo ?".
           05 FILLER PIC X(120) VALUE "写真に写っているのは誰？".
           05 FILLER PIC X(120) VALUE "照片里是谁？".
           *>   DROP-HINT
           05 FILLER PIC X(120) VALUE "Drop a photo here, or click to choose".
           05 FILLER PIC X(120) VALUE "Solte uma foto aqui ou clique para escolher".
           05 FILLER PIC X(120) VALUE "Suelta una foto aquí o haz clic para elegir".
           05 FILLER PIC X(120) VALUE "Déposez une photo ici ou cliquez pour choisir".
           05 FILLER PIC X(120) VALUE "写真をここにドロップするか、クリックして選択".
           05 FILLER PIC X(120) VALUE "将照片拖放到此处，或单击选择".
           *>   RECOGNIZE
           05 FILLER PIC X(120) VALUE "Recognize".
           05 FILLER PIC X(120) VALUE "Reconhecer".
           05 FILLER PIC X(120) VALUE "Reconocer".
           05 FILLER PIC X(120) VALUE "Reconnaître".
           05 FILLER PIC X(120) VALUE "認識".
           05 FILLER PIC X(120) VALUE "识别".
           *>   REK-MATCH
           05 FILLER PIC X(120) VALUE "Match: &1 %".
           05 FILLER PIC X(120) VALUE "Correspondência: &1 %".
           05 FILLER PIC X(120) VALUE "Coincidencia: &1 %".
           05 FILLER PIC X(120) VALUE "Correspondance : &1 %".
           05 FILLER PIC X(120) VALUE "一致度: &1 %".
           05 FILLER PIC X(120) VALUE "匹配度：&1 %".
           *>   CONFIRM-DELETE
           05 FILLER PIC X(120) VALUE "Delete this pet and its photo?".
           05 FILLER PIC X(120) VALUE "Excluir este animal e a foto dele?".
           05 FILLER PIC X(120) VALUE "¿Eliminar esta mascota y su foto?".
           05 FILLER PIC X(120) VALUE "Supprimer cet animal et sa photo ?".
           05 FILLER PIC X(120) VALUE "このペットとその写真を削除しますか？".
           05 FILLER PIC X(120) VALUE "要删除此宠物及其照片吗？".
           *>   NOT-CONNECTED
           05 FILLER PIC X(120) VALUE "Not connected".
           05 FILLER PIC X(120) VALUE "Não conectado".
           05 FILLER PIC X(120) VALUE "No conectado".
           05 FILLER PIC X(120) VALUE "Non connecté".
           05 FILLER PIC X(120) VALUE "未接続".
           05 FILLER PIC X(120) VALUE "未连接".
           *>   KEYS-REFUSED
           05 FILLER PIC X(120) VALUE "The keys were not accepted: &1".
           05 FILLER PIC X(120) VALUE "As chaves não foram aceitas: &1".
           05 FILLER PIC X(120) VALUE "No se aceptaron las claves: &1".
           05 FILLER PIC X(120) VALUE "Les clés n'ont pas été acceptées : &1".
           05 FILLER PIC X(120) VALUE "キーが受け付けられませんでした: &1".
           05 FILLER PIC X(120) VALUE "密钥未被接受：&1".
           *>   CHECKING-KEYS
           05 FILLER PIC X(120) VALUE "Checking the keys…".
           05 FILLER PIC X(120) VALUE "Verificando as chaves…".
           05 FILLER PIC X(120) VALUE "Comprobando las claves…".
           05 FILLER PIC X(120) VALUE "Vérification des clés…".
           05 FILLER PIC X(120) VALUE "キーを確認しています…".
           05 FILLER PIC X(120) VALUE "正在检查密钥…".
           *>   NOT-CONNECTED-WHY
           05 FILLER PIC X(120) VALUE "Not connected: &1".
           05 FILLER PIC X(120) VALUE "Não conectado: &1".
           05 FILLER PIC X(120) VALUE "No conectado: &1".
           05 FILLER PIC X(120) VALUE "Non connecté : &1".
           05 FILLER PIC X(120) VALUE "未接続: &1".
           05 FILLER PIC X(120) VALUE "未连接：&1".
           *>   CONNECTED-AS
           05 FILLER PIC X(120) VALUE "Connected as &1".
           05 FILLER PIC X(120) VALUE "Conectado como &1".
           05 FILLER PIC X(120) VALUE "Conectado como &1".
           05 FILLER PIC X(120) VALUE "Connecté en tant que &1".
           05 FILLER PIC X(120) VALUE "&1 として接続しました".
           05 FILLER PIC X(120) VALUE "已连接，身份为 &1".
           *>   NO-STACK
           05 FILLER PIC X(120) VALUE "No stack deployed in this session.".
           05 FILLER PIC X(120) VALUE "Nenhuma pilha implantada nesta sessão.".
           05 FILLER PIC X(120) VALUE "Ninguna pila desplegada en esta sesión.".
           05 FILLER PIC X(120) VALUE "Aucune pile déployée dans cette session.".
           05 FILLER PIC X(120) VALUE "このセッションでデプロイされたスタックはありません。".
           05 FILLER PIC X(120) VALUE "本次会话中尚未部署堆栈。".
           *>   STACK-WAIT
           05 FILLER PIC X(120) VALUE "Waiting for CloudFormation: &1".
           05 FILLER PIC X(120) VALUE "Aguardando o CloudFormation: &1".
           05 FILLER PIC X(120) VALUE "Esperando a CloudFormation: &1".
           05 FILLER PIC X(120) VALUE "En attente de CloudFormation : &1".
           05 FILLER PIC X(120) VALUE "CloudFormation を待っています: &1".
           05 FILLER PIC X(120) VALUE "正在等待 CloudFormation：&1".
           *>   STACK-STATE
           05 FILLER PIC X(120) VALUE "Stack &1: &2".
           05 FILLER PIC X(120) VALUE "Pilha &1: &2".
           05 FILLER PIC X(120) VALUE "Pila &1: &2".
           05 FILLER PIC X(120) VALUE "Pile &1 : &2".
           05 FILLER PIC X(120) VALUE "スタック &1: &2".
           05 FILLER PIC X(120) VALUE "堆栈 &1：&2".
           *>   TEMPLATE-UNREADABLE
           05 FILLER PIC X(120) VALUE "The template file could not be read: &1".
           05 FILLER PIC X(120) VALUE "Não foi possível ler o arquivo do modelo: &1".
           05 FILLER PIC X(120) VALUE "No se pudo leer el archivo de la plantilla: &1".
           05 FILLER PIC X(120) VALUE "Le fichier du modèle n'a pas pu être lu : &1".
           05 FILLER PIC X(120) VALUE "テンプレートファイルを読み込めませんでした: &1".
           05 FILLER PIC X(120) VALUE "无法读取模板文件：&1".
           *>   CREATING-STACK
           05 FILLER PIC X(120) VALUE "Creating the stack…".
           05 FILLER PIC X(120) VALUE "Criando a pilha…".
           05 FILLER PIC X(120) VALUE "Creando la pila…".
           05 FILLER PIC X(120) VALUE "Création de la pile…".
           05 FILLER PIC X(120) VALUE "スタックを作成しています…".
           05 FILLER PIC X(120) VALUE "正在创建堆栈…".
           *>   CF-REFUSED
           05 FILLER PIC X(120) VALUE "CloudFormation refused it: &1".
           05 FILLER PIC X(120) VALUE "O CloudFormation recusou: &1".
           05 FILLER PIC X(120) VALUE "CloudFormation lo rechazó: &1".
           05 FILLER PIC X(120) VALUE "CloudFormation l'a refusé : &1".
           05 FILLER PIC X(120) VALUE "CloudFormation に拒否されました: &1".
           05 FILLER PIC X(120) VALUE "CloudFormation 拒绝了请求：&1".
           *>   TEMPLATE-NOT-SAVED
           05 FILLER PIC X(120) VALUE "The template could not be saved.".
           05 FILLER PIC X(120) VALUE "Não foi possível salvar o modelo.".
           05 FILLER PIC X(120) VALUE "No se pudo guardar la plantilla.".
           05 FILLER PIC X(120) VALUE "Le modèle n'a pas pu être enregistré.".
           05 FILLER PIC X(120) VALUE "テンプレートを保存できませんでした。".
           05 FILLER PIC X(120) VALUE "无法保存模板。".
           *>   TEMPLATE-SAVED
           05 FILLER PIC X(120) VALUE "Template saved to &1".
           05 FILLER PIC X(120) VALUE "Modelo salvo em &1".
           05 FILLER PIC X(120) VALUE "Plantilla guardada en &1".
           05 FILLER PIC X(120) VALUE "Modèle enregistré dans &1".
           05 FILLER PIC X(120) VALUE "テンプレートを &1 に保存しました".
           05 FILLER PIC X(120) VALUE "模板已保存到 &1".
           *>   PRESS-RELOAD-TABLE
           05 FILLER PIC X(120) VALUE "Press Reload to read the table.".
           05 FILLER PIC X(120) VALUE "Pressione Recarregar para ler a tabela.".
           05 FILLER PIC X(120) VALUE "Pulsa Recargar para leer la tabla.".
           05 FILLER PIC X(120) VALUE "Appuyez sur Actualiser pour lire la table.".
           05 FILLER PIC X(120) VALUE "「再読み込み」を押してテーブルを読み込みます。".
           05 FILLER PIC X(120) VALUE "按“重新加载”读取该表。".
           *>   TABLE-EXISTS
           05 FILLER PIC X(120) VALUE "The table PowerCloudPets already exists.".
           05 FILLER PIC X(120) VALUE "A tabela PowerCloudPets já existe.".
           05 FILLER PIC X(120) VALUE "La tabla PowerCloudPets ya existe.".
           05 FILLER PIC X(120) VALUE "La table PowerCloudPets existe déjà.".
           05 FILLER PIC X(120) VALUE "テーブル PowerCloudPets は既に存在します。".
           05 FILLER PIC X(120) VALUE "表 PowerCloudPets 已存在。".
           *>   TABLE-CREATED
           05 FILLER PIC X(120) VALUE "The table PowerCloudPets was created.".
           05 FILLER PIC X(120) VALUE "A tabela PowerCloudPets foi criada.".
           05 FILLER PIC X(120) VALUE "Se creó la tabla PowerCloudPets.".
           05 FILLER PIC X(120) VALUE "La table PowerCloudPets a été créée.".
           05 FILLER PIC X(120) VALUE "テーブル PowerCloudPets を作成しました。".
           05 FILLER PIC X(120) VALUE "已创建表 PowerCloudPets。".
           *>   TABLE-NOT-CREATED
           05 FILLER PIC X(120) VALUE "The table could not be created: &1".
           05 FILLER PIC X(120) VALUE "Não foi possível criar a tabela: &1".
           05 FILLER PIC X(120) VALUE "No se pudo crear la tabla: &1".
           05 FILLER PIC X(120) VALUE "La table n'a pas pu être créée : &1".
           05 FILLER PIC X(120) VALUE "テーブルを作成できませんでした: &1".
           05 FILLER PIC X(120) VALUE "无法创建表：&1".
           *>   TABLE-NOT-READ
           05 FILLER PIC X(120) VALUE "The table could not be read: &1".
           05 FILLER PIC X(120) VALUE "Não foi possível ler a tabela: &1".
           05 FILLER PIC X(120) VALUE "No se pudo leer la tabla: &1".
           05 FILLER PIC X(120) VALUE "La table n'a pas pu être lue : &1".
           05 FILLER PIC X(120) VALUE "テーブルを読み込めませんでした: &1".
           05 FILLER PIC X(120) VALUE "无法读取该表：&1".
           *>   PETS-COUNT
           05 FILLER PIC X(120) VALUE "&1 pet(s) in the table.".
           05 FILLER PIC X(120) VALUE "&1 animal(is) na tabela.".
           05 FILLER PIC X(120) VALUE "&1 mascota(s) en la tabla.".
           05 FILLER PIC X(120) VALUE "&1 animal(aux) dans la table.".
           05 FILLER PIC X(120) VALUE "テーブルに &1 匹のペットがいます。".
           05 FILLER PIC X(120) VALUE "表中有 &1 只宠物。".
           *>   PHOTO-NOT-STORED
           05 FILLER PIC X(120) VALUE "The photo was not stored: &1".
           05 FILLER PIC X(120) VALUE "A foto não foi armazenada: &1".
           05 FILLER PIC X(120) VALUE "No se guardó la foto: &1".
           05 FILLER PIC X(120) VALUE "La photo n'a pas été enregistrée : &1".
           05 FILLER PIC X(120) VALUE "写真を保存できませんでした: &1".
           05 FILLER PIC X(120) VALUE "未能存储照片：&1".
           *>   PET-NOT-STORED
           05 FILLER PIC X(120) VALUE "The pet was not stored: &1".
           05 FILLER PIC X(120) VALUE "O animal não foi armazenado: &1".
           05 FILLER PIC X(120) VALUE "No se guardó la mascota: &1".
           05 FILLER PIC X(120) VALUE "L'animal n'a pas été enregistré : &1".
           05 FILLER PIC X(120) VALUE "ペットを保存できませんでした: &1".
           05 FILLER PIC X(120) VALUE "未能存储宠物：&1".
           *>   PET-GONE
           05 FILLER PIC X(120) VALUE "That pet is no longer in the table.".
           05 FILLER PIC X(120) VALUE "Esse animal não está mais na tabela.".
           05 FILLER PIC X(120) VALUE "Esa mascota ya no está en la tabla.".
           05 FILLER PIC X(120) VALUE "Cet animal n'est plus dans la table.".
           05 FILLER PIC X(120) VALUE "そのペットはもうテーブルにありません。".
           05 FILLER PIC X(120) VALUE "该宠物已不在表中。".
           *>   PET-NOT-DELETED
           05 FILLER PIC X(120) VALUE "The pet was not deleted: &1".
           05 FILLER PIC X(120) VALUE "O animal não foi excluído: &1".
           05 FILLER PIC X(120) VALUE "No se eliminó la mascota: &1".
           05 FILLER PIC X(120) VALUE "L'animal n'a pas été supprimé : &1".
           05 FILLER PIC X(120) VALUE "ペットを削除できませんでした: &1".
           05 FILLER PIC X(120) VALUE "未能删除宠物：&1".
           *>   PRESS-RELOAD-BUCKETS
           05 FILLER PIC X(120) VALUE "Press Reload to read the buckets.".
           05 FILLER PIC X(120) VALUE "Pressione Recarregar para ler os buckets.".
           05 FILLER PIC X(120) VALUE "Pulsa Recargar para leer los buckets.".
           05 FILLER PIC X(120) VALUE "Appuyez sur Actualiser pour lire les buckets.".
           05 FILLER PIC X(120) VALUE "「再読み込み」を押してバケットを読み込みます。".
           05 FILLER PIC X(120) VALUE "按“重新加载”读取存储桶。".
           *>   BUCKETS-NOT-LISTED
           05 FILLER PIC X(120) VALUE "The buckets could not be listed: &1".
           05 FILLER PIC X(120) VALUE "Não foi possível listar os buckets: &1".
           05 FILLER PIC X(120) VALUE "No se pudieron listar los buckets: &1".
           05 FILLER PIC X(120) VALUE "Les buckets n'ont pas pu être listés : &1".
           05 FILLER PIC X(120) VALUE "バケットを一覧表示できませんでした: &1".
           05 FILLER PIC X(120) VALUE "无法列出存储桶：&1".
           *>   NO-BUCKETS
           05 FILLER PIC X(120) VALUE "This account has no buckets in this region.".
           05 FILLER PIC X(120) VALUE "Esta conta não tem buckets nesta região.".
           05 FILLER PIC X(120) VALUE "Esta cuenta no tiene buckets en esta región.".
           05 FILLER PIC X(120) VALUE "Ce compte n'a aucun bucket dans cette région.".
           05 FILLER PIC X(120) VALUE "このアカウントには、このリージョンにバケットがありません。".
           05 FILLER PIC X(120) VALUE "此账户在该区域没有存储桶。".
           *>   BUCKET-NOT-LISTED
           05 FILLER PIC X(120) VALUE "The bucket could not be listed: &1".
           05 FILLER PIC X(120) VALUE "Não foi possível listar o bucket: &1".
           05 FILLER PIC X(120) VALUE "No se pudo listar el bucket: &1".
           05 FILLER PIC X(120) VALUE "Le bucket n'a pas pu être listé : &1".
           05 FILLER PIC X(120) VALUE "バケットの内容を一覧表示できませんでした: &1".
           05 FILLER PIC X(120) VALUE "无法列出该存储桶：&1".
           *>   OBJECTS-COUNT
           05 FILLER PIC X(120) VALUE "&1 object(s) in &2".
           05 FILLER PIC X(120) VALUE "&1 objeto(s) em &2".
           05 FILLER PIC X(120) VALUE "&1 objeto(s) en &2".
           05 FILLER PIC X(120) VALUE "&1 objet(s) dans &2".
           05 FILLER PIC X(120) VALUE "&2 に &1 個のオブジェクト".
           05 FILLER PIC X(120) VALUE "&2 中有 &1 个对象".
           *>   NOTHING-ASKED
           05 FILLER PIC X(120) VALUE "Nothing asked yet.".
           05 FILLER PIC X(120) VALUE "Nada foi perguntado ainda.".
           05 FILLER PIC X(120) VALUE "Aún no se ha preguntado nada.".
           05 FILLER PIC X(120) VALUE "Rien n'a encore été demandé.".
           05 FILLER PIC X(120) VALUE "まだ何も尋ねていません。".
           05 FILLER PIC X(120) VALUE "尚未发出任何请求。".
           *>   FUNCTION-NOT-CALLED
           05 FILLER PIC X(120) VALUE "The function could not be called: &1".
           05 FILLER PIC X(120) VALUE "Não foi possível chamar a função: &1".
           05 FILLER PIC X(120) VALUE "No se pudo llamar a la función: &1".
           05 FILLER PIC X(120) VALUE "La fonction n'a pas pu être appelée : &1".
           05 FILLER PIC X(120) VALUE "関数を呼び出せませんでした: &1".
           05 FILLER PIC X(120) VALUE "无法调用该函数：&1".
           *>   FUNCTION-ANSWERED
           05 FILLER PIC X(120) VALUE "The function answered: &1".
           05 FILLER PIC X(120) VALUE "A função respondeu: &1".
           05 FILLER PIC X(120) VALUE "La función respondió: &1".
           05 FILLER PIC X(120) VALUE "La fonction a répondu : &1".
           05 FILLER PIC X(120) VALUE "関数の応答: &1".
           05 FILLER PIC X(120) VALUE "函数返回：&1".
           *>   LINK-RESULT
           05 FILLER PIC X(120) VALUE "Link: &1?... (signature hidden)".
           05 FILLER PIC X(120) VALUE "Link: &1?... (assinatura oculta)".
           05 FILLER PIC X(120) VALUE "Enlace: &1?... (firma oculta)".
           05 FILLER PIC X(120) VALUE "Lien : &1?... (signature masquée)".
           05 FILLER PIC X(120) VALUE "リンク: &1?...（署名は非表示）".
           05 FILLER PIC X(120) VALUE "链接：&1?...（签名已隐藏）".
           *>   TYPE-AND-SPEAK
           05 FILLER PIC X(120) VALUE "Type a text and press Speak.".
           05 FILLER PIC X(120) VALUE "Digite um texto e pressione Falar.".
           05 FILLER PIC X(120) VALUE "Escribe un texto y pulsa Hablar.".
           05 FILLER PIC X(120) VALUE "Saisissez un texte et appuyez sur Parler.".
           05 FILLER PIC X(120) VALUE "テキストを入力して「読み上げ」を押してください。".
           05 FILLER PIC X(120) VALUE "输入文字，然后按“朗读”。".
           *>   ASKING-POLLY
           05 FILLER PIC X(120) VALUE "Asking Polly…".
           05 FILLER PIC X(120) VALUE "Consultando o Polly…".
           05 FILLER PIC X(120) VALUE "Consultando a Polly…".
           05 FILLER PIC X(120) VALUE "Demande à Polly…".
           05 FILLER PIC X(120) VALUE "Polly に問い合わせています…".
           05 FILLER PIC X(120) VALUE "正在请求 Polly…".
           *>   POLLY-FAILED
           05 FILLER PIC X(120) VALUE "Polly could not speak: &1".
           05 FILLER PIC X(120) VALUE "O Polly não conseguiu falar: &1".
           05 FILLER PIC X(120) VALUE "Polly no pudo hablar: &1".
           05 FILLER PIC X(120) VALUE "Polly n'a pas pu parler : &1".
           05 FILLER PIC X(120) VALUE "Polly が読み上げできませんでした: &1".
           05 FILLER PIC X(120) VALUE "Polly 无法朗读：&1".
           *>   POLLY-READY
           05 FILLER PIC X(120) VALUE "Ready: &1 characters.".
           05 FILLER PIC X(120) VALUE "Pronto: &1 caracteres.".
           05 FILLER PIC X(120) VALUE "Listo: &1 caracteres.".
           05 FILLER PIC X(120) VALUE "Prêt : &1 caractères.".
           05 FILLER PIC X(120) VALUE "準備完了: &1 文字。".
           05 FILLER PIC X(120) VALUE "就绪：&1 个字符。".
           *>   PICK-REGION
           05 FILLER PIC X(120) VALUE "Pick a region or press Reload.".
           05 FILLER PIC X(120) VALUE "Escolha uma região ou pressione Recarregar.".
           05 FILLER PIC X(120) VALUE "Elige una región o pulsa Recargar.".
           05 FILLER PIC X(120) VALUE "Choisissez une région ou appuyez sur Actualiser.".
           05 FILLER PIC X(120) VALUE "リージョンを選ぶか「再読み込み」を押してください。".
           05 FILLER PIC X(120) VALUE "选择区域，或按“重新加载”。".
           *>   SERVERS-NOT-LISTED
           05 FILLER PIC X(120) VALUE "The servers could not be listed: &1".
           05 FILLER PIC X(120) VALUE "Não foi possível listar os servidores: &1".
           05 FILLER PIC X(120) VALUE "No se pudieron listar los servidores: &1".
           05 FILLER PIC X(120) VALUE "Les serveurs n'ont pas pu être listés : &1".
           05 FILLER PIC X(120) VALUE "サーバーを一覧表示できませんでした: &1".
           05 FILLER PIC X(120) VALUE "无法列出服务器：&1".
           *>   SERVERS-COUNT
           05 FILLER PIC X(120) VALUE "&1 server(s) in &2".
           05 FILLER PIC X(120) VALUE "&1 servidor(es) em &2".
           05 FILLER PIC X(120) VALUE "&1 servidor(es) en &2".
           05 FILLER PIC X(120) VALUE "&1 serveur(s) dans &2".
           05 FILLER PIC X(120) VALUE "&2 に &1 台のサーバー".
           05 FILLER PIC X(120) VALUE "&2 中有 &1 台服务器".
           *>   AWS-REFUSED
           05 FILLER PIC X(120) VALUE "AWS refused: &1".
           05 FILLER PIC X(120) VALUE "A AWS recusou: &1".
           05 FILLER PIC X(120) VALUE "AWS lo rechazó: &1".
           05 FILLER PIC X(120) VALUE "AWS l'a refusé : &1".
           05 FILLER PIC X(120) VALUE "AWS に拒否されました: &1".
           05 FILLER PIC X(120) VALUE "AWS 拒绝了请求：&1".
           *>   REK-INITIAL
           05 FILLER PIC X(120) VALUE "Drop a photo, then press Recognize.".
           05 FILLER PIC X(120) VALUE "Solte uma foto e pressione Reconhecer.".
           05 FILLER PIC X(120) VALUE "Suelta una foto y pulsa Reconocer.".
           05 FILLER PIC X(120) VALUE "Déposez une photo, puis appuyez sur Reconnaître.".
           05 FILLER PIC X(120) VALUE "写真をドロップして「認識」を押してください。".
           05 FILLER PIC X(120) VALUE "拖放一张照片，然后按“识别”。".
           *>   REK-DROP-FIRST
           05 FILLER PIC X(120) VALUE "Drop a photo first.".
           05 FILLER PIC X(120) VALUE "Solte uma foto primeiro.".
           05 FILLER PIC X(120) VALUE "Suelta primero una foto.".
           05 FILLER PIC X(120) VALUE "Déposez d'abord une photo.".
           05 FILLER PIC X(120) VALUE "先に写真をドロップしてください。".
           05 FILLER PIC X(120) VALUE "请先拖放一张照片。".
           *>   REK-ASKING
           05 FILLER PIC X(120) VALUE "Asking Rekognition…".
           05 FILLER PIC X(120) VALUE "Consultando o Rekognition…".
           05 FILLER PIC X(120) VALUE "Consultando a Rekognition…".
           05 FILLER PIC X(120) VALUE "Demande à Rekognition…".
           05 FILLER PIC X(120) VALUE "Rekognition に問い合わせています…".
           05 FILLER PIC X(120) VALUE "正在请求 Rekognition…".
           *>   REK-SAID
           05 FILLER PIC X(120) VALUE "Rekognition said: &1".
           05 FILLER PIC X(120) VALUE "O Rekognition respondeu: &1".
           05 FILLER PIC X(120) VALUE "Rekognition respondió: &1".
           05 FILLER PIC X(120) VALUE "Rekognition a répondu : &1".
           05 FILLER PIC X(120) VALUE "Rekognition の応答: &1".
           05 FILLER PIC X(120) VALUE "Rekognition 返回：&1".
           *>   REK-NONE
           05 FILLER PIC X(120) VALUE "No celebrity was recognised in this picture.".
           05 FILLER PIC X(120) VALUE "Nenhuma celebridade foi reconhecida nesta imagem.".
           05 FILLER PIC X(120) VALUE "No se reconoció a ninguna celebridad en esta imagen.".
           05 FILLER PIC X(120) VALUE "Aucune célébrité n'a été reconnue sur cette image.".
           05 FILLER PIC X(120) VALUE "この画像で有名人は認識されませんでした。".
           05 FILLER PIC X(120) VALUE "此图片中未识别出名人。".
           *>   REK-PRESS
           05 FILLER PIC X(120) VALUE "Press Recognize.".
           05 FILLER PIC X(120) VALUE "Pressione Reconhecer.".
           05 FILLER PIC X(120) VALUE "Pulsa Reconocer.".
           05 FILLER PIC X(120) VALUE "Appuyez sur Reconnaître.".
           05 FILLER PIC X(120) VALUE "「認識」を押してください。".
           05 FILLER PIC X(120) VALUE "请按“识别”。".
           *>   NO-BUCKET
           05 FILLER PIC X(120) VALUE "No photo bucket yet: deploy the stack on the Settings tab, or type the bucket's name there.".
           05 FILLER PIC X(120) VALUE "Ainda não há bucket das fotos: implante a pilha em Configurações ou digite ali o nome do bucket.".
           05 FILLER PIC X(120) VALUE "Aún no hay bucket de fotos: implemente la pila en Configuración o escriba allí el nombre del bucket.".
           05 FILLER PIC X(120) VALUE "Pas encore de bucket des photos : déployez la pile dans Paramètres, ou tapez-y le nom du bucket.".
           05 FILLER PIC X(120) VALUE "写真バケットがありません。設定タブでデプロイするか名前を入力してください。".
           05 FILLER PIC X(120) VALUE "尚无照片存储桶：请在“设置”中部署堆栈或输入存储桶名称。".
           *>   CF-NO-RIGHTS
           05 FILLER PIC X(120) VALUE "These keys cannot deploy the stack: deploy with an administrator's keys, or Save template… and run it; then Connect.".
           05 FILLER PIC X(120) VALUE "Estas chaves não implantam a pilha: implante com chaves de administrador ou Salvar modelo…; depois Conectar.".
           05 FILLER PIC X(120) VALUE "Estas claves no implementan la pila: hágalo con claves de administrador o Guardar plantilla…; luego Conectar.".
           05 FILLER PIC X(120) VALUE "Ces clés ne déploient pas : prenez des clés d'administrateur ou Enregistrer le modèle…, puis Se connecter.".
           05 FILLER PIC X(120) VALUE "このキーではデプロイ不可。管理者キーか「テンプレートを保存」で行い、接続。".
           05 FILLER PIC X(120) VALUE "这些密钥无法部署：请用管理员密钥，或“保存模板…”后自行部署，再连接。".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
           05 PC-TEXT-ROW OCCURS 110.
               10 PC-TEXT PIC X(120) OCCURS 6.
       *>   The same texts in the language picked, by name.

       01 PC-TEXTS-NOW GLOBAL.
           05 T-TAGLINE PIC X(120).
           05 T-LANG-TIP PIC X(120).
           05 T-TAB-SETTINGS PIC X(120).
           05 T-SET-HEAD PIC X(120).
           05 T-REGION PIC X(120).
           05 T-KEY-ID PIC X(120).
           05 T-TOKEN PIC X(120).
           05 T-TOKEN-HINT PIC X(120).
           05 T-SECRET PIC X(120).
           05 T-CONNECT PIC X(120).
           05 T-STACK-NAME PIC X(120).
           05 T-BUCKET PIC X(120).
           05 T-BUCKET-HINT PIC X(120).
           05 T-FUNCTION PIC X(120).
           05 T-DEPLOY PIC X(120).
           05 T-SAVE-TEMPLATE PIC X(120).
           05 T-LINK-TEMPLATE PIC X(120).
           05 T-SAVE-DIALOG PIC X(120).
           05 T-DDB-HEAD PIC X(120).
           05 T-CREATE-TABLE PIC X(120).
           05 T-NEW-PET PIC X(120).
           05 T-RELOAD PIC X(120).
           05 T-CLEAR-FILTERS PIC X(120).
           05 T-COL-ID PIC X(120).
           05 T-COL-PHOTO PIC X(120).
           05 T-COL-NAME PIC X(120).
           05 T-COL-RACE PIC X(120).
           05 T-COL-AGE PIC X(120).
           05 T-COL-VIEW PIC X(120).
           05 T-COL-EDIT PIC X(120).
           05 T-COL-DELETE PIC X(120).
           05 T-S3-HEAD PIC X(120).
           05 T-COL-KEY PIC X(120).
           05 T-COL-SIZE PIC X(120).
           05 T-COL-MODIFIED PIC X(120).
           05 T-LAM-HEAD PIC X(120).
           05 T-LAM-SRC PIC X(120).
           05 T-LAM-KEY PIC X(120).
           05 T-LAM-TRY PIC X(120).
           05 T-POLLY-HEAD PIC X(120).
           05 T-POLLY-LANG PIC X(120).
           05 T-VOICE-1 PIC X(120).
           05 T-VOICE-2 PIC X(120).
           05 T-VOICE-3 PIC X(120).
           05 T-VOICE-4 PIC X(120).
           05 T-VOICE-5 PIC X(120).
           05 T-VOICE-6 PIC X(120).
           05 T-SPEAK PIC X(120).
           05 T-EC2-HEAD PIC X(120).
           05 T-COL-INSTANCE PIC X(120).
           05 T-COL-CLASS PIC X(120).
           05 T-COL-VCPU PIC X(120).
           05 T-COL-MEMORY PIC X(120).
           05 T-COL-STATE PIC X(120).
           05 T-COL-POWER PIC X(120).
           05 T-WAIT-RUNNING PIC X(120).
           05 T-WAIT-STOPPED PIC X(120).
           05 T-REK-HEAD PIC X(120).
           05 T-DROP-HINT PIC X(120).
           05 T-RECOGNIZE PIC X(120).
           05 T-REK-MATCH PIC X(120).
           05 T-CONFIRM-DELETE PIC X(120).
           05 T-NOT-CONNECTED PIC X(120).
           05 T-KEYS-REFUSED PIC X(120).
           05 T-CHECKING-KEYS PIC X(120).
           05 T-NOT-CONNECTED-WHY PIC X(120).
           05 T-CONNECTED-AS PIC X(120).
           05 T-NO-STACK PIC X(120).
           05 T-STACK-WAIT PIC X(120).
           05 T-STACK-STATE PIC X(120).
           05 T-TEMPLATE-UNREADABLE PIC X(120).
           05 T-CREATING-STACK PIC X(120).
           05 T-CF-REFUSED PIC X(120).
           05 T-TEMPLATE-NOT-SAVED PIC X(120).
           05 T-TEMPLATE-SAVED PIC X(120).
           05 T-PRESS-RELOAD-TABLE PIC X(120).
           05 T-TABLE-EXISTS PIC X(120).
           05 T-TABLE-CREATED PIC X(120).
           05 T-TABLE-NOT-CREATED PIC X(120).
           05 T-TABLE-NOT-READ PIC X(120).
           05 T-PETS-COUNT PIC X(120).
           05 T-PHOTO-NOT-STORED PIC X(120).
           05 T-PET-NOT-STORED PIC X(120).
           05 T-PET-GONE PIC X(120).
           05 T-PET-NOT-DELETED PIC X(120).
           05 T-PRESS-RELOAD-BUCKETS PIC X(120).
           05 T-BUCKETS-NOT-LISTED PIC X(120).
           05 T-NO-BUCKETS PIC X(120).
           05 T-BUCKET-NOT-LISTED PIC X(120).
           05 T-OBJECTS-COUNT PIC X(120).
           05 T-NOTHING-ASKED PIC X(120).
           05 T-FUNCTION-NOT-CALLED PIC X(120).
           05 T-FUNCTION-ANSWERED PIC X(120).
           05 T-LINK-RESULT PIC X(120).
           05 T-TYPE-AND-SPEAK PIC X(120).
           05 T-ASKING-POLLY PIC X(120).
           05 T-POLLY-FAILED PIC X(120).
           05 T-POLLY-READY PIC X(120).
           05 T-PICK-REGION PIC X(120).
           05 T-SERVERS-NOT-LISTED PIC X(120).
           05 T-SERVERS-COUNT PIC X(120).
           05 T-AWS-REFUSED PIC X(120).
           05 T-REK-INITIAL PIC X(120).
           05 T-REK-DROP-FIRST PIC X(120).
           05 T-REK-ASKING PIC X(120).
           05 T-REK-SAID PIC X(120).
           05 T-REK-NONE PIC X(120).
           05 T-REK-PRESS PIC X(120).
           05 T-NO-BUCKET PIC X(120).
           05 T-CF-NO-RIGHTS PIC X(120).
       01 PC-TEXTS-NOW-R REDEFINES PC-TEXTS-NOW GLOBAL.
           05 PC-TEXT-NOW PIC X(120) OCCURS 110.
       *>   The two paragraphs that do not fit the short texts.

       01 PC-LONG-DATA GLOBAL.
           *>   SET-HINT
           05 FILLER PIC X(800) VALUE "Type the keys of an AWS user. They stay in memory for this run only and are cleared from these boxes as soon as you press Connect. Leave them empty to use the default AWS profile of this machine.".
           05 FILLER PIC X(800) VALUE "Digite as chaves de um usuário da AWS. Elas ficam na memória somente nesta execução e são apagadas destas caixas assim que você pressiona Conectar. Deixe-as vazias para usar o perfil padrão da AWS desta máquina.".
           05 FILLER PIC X(800) VALUE "Escribe las claves de un usuario de AWS. Solo permanecen en memoria durante esta ejecución y se borran de estas casillas en cuanto pulsas Conectar. Déjalas vacías para usar el perfil de AWS predeterminado de este equipo.".
           05 FILLER PIC X(800) VALUE "Saisissez les clés d'un utilisateur AWS. Elles ne restent en mémoire que pendant cette exécution et sont effacées de ces champs dès que vous appuyez sur Se connecter. Laissez-les vides pour utiliser le profil AWS par défaut de cette machine.".
           05 FILLER PIC X(800) VALUE "AWS ユーザーのキーを入力してください。キーはこの実行中だけメモリに保持され、「接続」を押すとすぐにこれらの入力欄から消去されます。空のままにすると、このマシンの既定の AWS プロファイルが使われます。".
           05 FILLER PIC X(800) VALUE "请输入某个 AWS 用户的密钥。密钥仅在本次运行期间保留在内存中，按下“连接”后会立即从这些输入框中清除。留空则使用本机默认的 AWS 配置文件。".
           *>   LAM-TEXT
           05 FILLER PIC X(800) VALUE "PowerCloud never signs a download link itself. The function powercloud-presign does: given the key of one photo, it answers a link to that object that works for 15 minutes, signed with a role that can only read the photo bucket. Use it to hand a photo to someone outside the application, in a browser or an e-mail. The application itself reads its photos through AWS's MCP server, as it makes every other AWS call; it never fetches the bytes behind a link. AwsLambda.Invoke needs AllowWrite on, because a function can do whatever its code says.".
           05 FILLER PIC X(800) VALUE "O PowerCloud nunca assina por conta própria um link de download. Quem assina é a função powercloud-presign: dada a chave de uma foto, ela responde um link para esse objeto que vale por 15 minutos, assinado com um papel que só pode ler o bucket das fotos. Use-a para entregar uma foto a alguém fora do aplicativo, num navegador ou num e-mail. O próprio aplicativo lê suas fotos pelo servidor MCP da AWS, como faz todas as outras chamadas à AWS; ele nunca busca os bytes por trás de um link. AwsLambda.Invoke exige AllowWrite ativado, porque uma função pode fazer tudo o que o seu código disser.".
           05 FILLER PIC X(800) VALUE "PowerCloud nunca firma por sí mismo un enlace de descarga. Lo firma la función powercloud-presign: dada la clave de una foto, responde un enlace a ese objeto que vale 15 minutos, firmado con un rol que solo puede leer el bucket de fotos. Úsala para pasar una foto a alguien fuera de la aplicación, en un navegador o un correo. La propia aplicación lee sus fotos a través del servidor MCP de AWS, como hace todas las demás llamadas a AWS; nunca descarga los bytes de un enlace. AwsLambda.Invoke necesita AllowWrite activado, porque una función puede hacer lo que diga su código.".
           05 FILLER PIC X(800) VALUE "PowerCloud ne signe jamais lui-même un lien de téléchargement. C'est la fonction powercloud-presign qui le signe : à partir de la clé d'une photo, elle renvoie un lien vers cet objet valable 15 minutes, signé avec un rôle qui ne peut que lire le bucket des photos. Servez-vous-en pour transmettre une photo à quelqu'un hors de l'application, dans un navigateur ou un e-mail. L'application lit elle-même ses photos par le serveur MCP d'AWS, comme tous ses autres appels à AWS ; elle ne récupère jamais les octets derrière un lien. AwsLambda.Invoke exige AllowWrite activé, car une fonction peut faire tout ce que dit son code.".
           05 FILLER PIC X(800) VALUE "PowerCloud は、ダウンロード用リンクを自分で署名することはありません。署名するのは関数 powercloud-presign です。写真のキーを渡すと、そのオブジェクトへの 15 分間有効なリンクを、写真バケットの読み取りしかできないロールで署名して返します。ブラウザーやメールで、アプリの外の人に写真を渡すときに使います。アプリ自身は、他のすべての AWS 呼び出しと同じく、AWS の MCP サーバーを通して写真を読みます。リンクの先のバイト列を取得することはありません。関数はコードに書かれたことを何でも実行できるため、AwsLambda.Invoke には AllowWrite をオンにする必要があります。".
           05 FILLER PIC X(800) VALUE "PowerCloud 从不自己对下载链接签名，签名由函数 powercloud-presign 完成：给它一张照片的键，它就返回指向该对象、有效期 15 分钟的链接，并用只能读取照片存储桶的角色签名。可用它把照片交给应用程序之外的人，在浏览器或电子邮件中打开。应用程序本身和它的其他所有 AWS 调用一样，通过 AWS 的 MCP 服务器读取照片，从不获取链接背后的字节。AwsLambda.Invoke 需要打开 AllowWrite，因为函数可以执行其代码所写的任何操作。".
       01 PC-LONG-TABLE REDEFINES PC-LONG-DATA GLOBAL.
           05 PC-LONG-ROW OCCURS 2.
               10 PC-LONG PIC X(800) OCCURS 6.
       01 T-SET-HINT PIC X(800) GLOBAL.
       01 T-LAM-TEXT PIC X(800) GLOBAL.
       *>   The row of each status message in PC-TEXT, so a status line can name it.

       01 M-NOT-CONNECTED         PIC 999     GLOBAL VALUE 63.
       01 M-KEYS-REFUSED          PIC 999     GLOBAL VALUE 64.
       01 M-CHECKING-KEYS         PIC 999     GLOBAL VALUE 65.
       01 M-NOT-CONNECTED-WHY     PIC 999     GLOBAL VALUE 66.
       01 M-CONNECTED-AS          PIC 999     GLOBAL VALUE 67.
       01 M-NO-STACK              PIC 999     GLOBAL VALUE 68.
       01 M-STACK-WAIT            PIC 999     GLOBAL VALUE 69.
       01 M-STACK-STATE           PIC 999     GLOBAL VALUE 70.
       01 M-TEMPLATE-UNREADABLE   PIC 999     GLOBAL VALUE 71.
       01 M-CREATING-STACK        PIC 999     GLOBAL VALUE 72.
       01 M-CF-REFUSED            PIC 999     GLOBAL VALUE 73.
       01 M-TEMPLATE-NOT-SAVED    PIC 999     GLOBAL VALUE 74.
       01 M-TEMPLATE-SAVED        PIC 999     GLOBAL VALUE 75.
       01 M-PRESS-RELOAD-TABLE    PIC 999     GLOBAL VALUE 76.
       01 M-TABLE-EXISTS          PIC 999     GLOBAL VALUE 77.
       01 M-TABLE-CREATED         PIC 999     GLOBAL VALUE 78.
       01 M-TABLE-NOT-CREATED     PIC 999     GLOBAL VALUE 79.
       01 M-TABLE-NOT-READ        PIC 999     GLOBAL VALUE 80.
       01 M-PETS-COUNT            PIC 999     GLOBAL VALUE 81.
       01 M-PHOTO-NOT-STORED      PIC 999     GLOBAL VALUE 82.
       01 M-NO-BUCKET             PIC 999     GLOBAL VALUE 109.
       01 M-CF-NO-RIGHTS          PIC 999     GLOBAL VALUE 110.
       01 M-PET-NOT-STORED        PIC 999     GLOBAL VALUE 83.
       01 M-PET-GONE              PIC 999     GLOBAL VALUE 84.
       01 M-PET-NOT-DELETED       PIC 999     GLOBAL VALUE 85.
       01 M-PRESS-RELOAD-BUCKETS  PIC 999     GLOBAL VALUE 86.
       01 M-BUCKETS-NOT-LISTED    PIC 999     GLOBAL VALUE 87.
       01 M-NO-BUCKETS            PIC 999     GLOBAL VALUE 88.
       01 M-BUCKET-NOT-LISTED     PIC 999     GLOBAL VALUE 89.
       01 M-OBJECTS-COUNT         PIC 999     GLOBAL VALUE 90.
       01 M-NOTHING-ASKED         PIC 999     GLOBAL VALUE 91.
       01 M-FUNCTION-NOT-CALLED   PIC 999     GLOBAL VALUE 92.
       01 M-FUNCTION-ANSWERED     PIC 999     GLOBAL VALUE 93.
       01 M-LINK-RESULT           PIC 999     GLOBAL VALUE 94.
       01 M-TYPE-AND-SPEAK        PIC 999     GLOBAL VALUE 95.
       01 M-ASKING-POLLY          PIC 999     GLOBAL VALUE 96.
       01 M-POLLY-FAILED          PIC 999     GLOBAL VALUE 97.
       01 M-POLLY-READY           PIC 999     GLOBAL VALUE 98.
       01 M-PICK-REGION           PIC 999     GLOBAL VALUE 99.
       01 M-SERVERS-NOT-LISTED    PIC 999     GLOBAL VALUE 100.
       01 M-SERVERS-COUNT         PIC 999     GLOBAL VALUE 101.
       01 M-AWS-REFUSED           PIC 999     GLOBAL VALUE 102.
       01 M-REK-INITIAL           PIC 999     GLOBAL VALUE 103.
       01 M-REK-DROP-FIRST        PIC 999     GLOBAL VALUE 104.
       01 M-REK-ASKING            PIC 999     GLOBAL VALUE 105.
       01 M-REK-SAID              PIC 999     GLOBAL VALUE 106.
       01 M-REK-NONE              PIC 999     GLOBAL VALUE 107.
       01 M-REK-PRESS             PIC 999     GLOBAL VALUE 108.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Pnl-Title.
          05 WS-Pnl-Title-TEXT       PIC X(256) VALUE 'Pnl-Title'.
          05 WS-Pnl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pnl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Brand.
          05 WS-Lbl-Brand-TEXT       PIC X(256) VALUE 'PowerCloud'.
          05 WS-Lbl-Brand-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Brand-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Tagline.
          05 WS-Lbl-Tagline-TEXT       PIC X(256) VALUE 'One AWS service on every tab, written in RustCOBOL'.
          05 WS-Lbl-Tagline-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Tagline-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Close.
          05 WS-Btn-Close-TEXT       PIC X(256) VALUE ''.
          05 WS-Btn-Close-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Close-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Language.
          05 WS-Cmb-Language-TEXT       PIC X(256) VALUE 'Cmb-Language'.
          05 WS-Cmb-Language-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Language-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Language-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Tab-Main.
          05 WS-Tab-Main-TEXT       PIC X(256) VALUE 'Tab-Main'.
          05 WS-Tab-Main-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tab-Main-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-Mcp.
          05 WS-Aws-Mcp-TEXT       PIC X(256) VALUE 'Aws-Mcp'.
          05 WS-Aws-Mcp-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-Mcp-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-Ddb.
          05 WS-Aws-Ddb-TEXT       PIC X(256) VALUE 'Aws-Ddb'.
          05 WS-Aws-Ddb-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-Ddb-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-S3.
          05 WS-Aws-S3-TEXT       PIC X(256) VALUE 'Aws-S3'.
          05 WS-Aws-S3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-S3-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-S3-List.
          05 WS-Aws-S3-List-TEXT       PIC X(256) VALUE 'Aws-S3-List'.
          05 WS-Aws-S3-List-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-S3-List-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-Lambda.
          05 WS-Aws-Lambda-TEXT       PIC X(256) VALUE 'Aws-Lambda'.
          05 WS-Aws-Lambda-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-Lambda-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-Polly.
          05 WS-Aws-Polly-TEXT       PIC X(256) VALUE 'Aws-Polly'.
          05 WS-Aws-Polly-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-Polly-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-Ec2.
          05 WS-Aws-Ec2-TEXT       PIC X(256) VALUE 'Aws-Ec2'.
          05 WS-Aws-Ec2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-Ec2-ENABLED    PIC 9      VALUE 1.

       01 WS-Aws-Rek.
          05 WS-Aws-Rek-TEXT       PIC X(256) VALUE 'Aws-Rek'.
          05 WS-Aws-Rek-VISIBLE    PIC 9      VALUE 1.
          05 WS-Aws-Rek-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Stack.
          05 WS-Tmr-Stack-TEXT       PIC X(256) VALUE 'Tmr-Stack'.
          05 WS-Tmr-Stack-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Stack-ENABLED    PIC 9      VALUE 1.

       01 WS-Pic-Wheel.
          05 WS-Pic-Wheel-TEXT       PIC X(256) VALUE 'Pic-Wheel'.
          05 WS-Pic-Wheel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pic-Wheel-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-SetHead.
          05 WS-Lbl-SetHead-TEXT       PIC X(256) VALUE 'Cloud access'.
          05 WS-Lbl-SetHead-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-SetHead-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-SetHint.
          05 WS-Lbl-SetHint-TEXT       PIC X(256) VALUE 'Type the keys of an AWS user. They stay in memory for this run only and are cleared from these boxes as soon as you press Connect. Leave them empty to use the default AWS profile of this machine.'.
          05 WS-Lbl-SetHint-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-SetHint-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Region.
          05 WS-Lbl-Region-TEXT       PIC X(256) VALUE 'Region'.
          05 WS-Lbl-Region-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Region-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Region.
          05 WS-Cmb-Region-TEXT       PIC X(256) VALUE 'Cmb-Region'.
          05 WS-Cmb-Region-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Region-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Region-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-KeyId.
          05 WS-Lbl-KeyId-TEXT       PIC X(256) VALUE 'Access key id'.
          05 WS-Lbl-KeyId-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-KeyId-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-KeyId.
          05 WS-Txt-KeyId-TEXT       PIC X(40) VALUE SPACES.
          05 WS-Txt-KeyId-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-KeyId-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-KeyId-VALUE      PIC X(40) VALUE SPACES.

       01 WS-Lbl-Token.
          05 WS-Lbl-Token-TEXT       PIC X(256) VALUE 'Session token (optional)'.
          05 WS-Lbl-Token-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Token-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Token.
          05 WS-Txt-Token-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Token-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Token-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Token-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Secret.
          05 WS-Lbl-Secret-TEXT       PIC X(256) VALUE 'Secret access key'.
          05 WS-Lbl-Secret-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Secret-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Secret.
          05 WS-Txt-Secret-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Secret-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Secret-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Secret-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Btn-Connect.
          05 WS-Btn-Connect-TEXT       PIC X(256) VALUE 'Connect'.
          05 WS-Btn-Connect-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Connect-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Conn.
          05 WS-Lbl-Conn-TEXT       PIC X(256) VALUE 'Not connected'.
          05 WS-Lbl-Conn-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Conn-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-StackHead.
          05 WS-Lbl-StackHead-TEXT       PIC X(256) VALUE 'CloudFormation'.
          05 WS-Lbl-StackHead-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-StackHead-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Stack.
          05 WS-Lbl-Stack-TEXT       PIC X(256) VALUE 'Stack name'.
          05 WS-Lbl-Stack-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Stack-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Stack.
          05 WS-Txt-Stack-TEXT       PIC X(256) VALUE 'powercloud'.
          05 WS-Txt-Stack-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Stack-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Stack-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Bucket.
          05 WS-Lbl-Bucket-TEXT       PIC X(256) VALUE 'Photo bucket'.
          05 WS-Lbl-Bucket-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Bucket-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Bucket.
          05 WS-Txt-Bucket-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Bucket-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Bucket-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Bucket-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Function.
          05 WS-Lbl-Function-TEXT       PIC X(256) VALUE 'Function'.
          05 WS-Lbl-Function-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Function-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Function.
          05 WS-Txt-Function-TEXT       PIC X(256) VALUE 'powercloud-presign'.
          05 WS-Txt-Function-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Function-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Function-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Btn-Deploy.
          05 WS-Btn-Deploy-TEXT       PIC X(256) VALUE 'Deploy CloudFormation'.
          05 WS-Btn-Deploy-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Deploy-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save.
          05 WS-Btn-Save-TEXT       PIC X(256) VALUE 'Save template…'.
          05 WS-Btn-Save-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Link.
          05 WS-Lbl-Link-TEXT       PIC X(256) VALUE 'Download the CloudFormation template'.
          05 WS-Lbl-Link-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Link-ENABLED    PIC 9      VALUE 1.

       01 WS-Pgb-Stack.
          05 WS-Pgb-Stack-TEXT       PIC X(256) VALUE 'Pgb-Stack'.
          05 WS-Pgb-Stack-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pgb-Stack-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-StackState.
          05 WS-Lbl-StackState-TEXT       PIC X(256) VALUE 'No stack deployed in this session.'.
          05 WS-Lbl-StackState-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-StackState-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Thumbs.
          05 WS-Tmr-Thumbs-TEXT       PIC X(256) VALUE 'Tmr-Thumbs'.
          05 WS-Tmr-Thumbs-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Thumbs-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-DdbHead.
          05 WS-Lbl-DdbHead-TEXT       PIC X(256) VALUE 'Pets'.
          05 WS-Lbl-DdbHead-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-DdbHead-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-CreateTable.
          05 WS-Btn-CreateTable-TEXT       PIC X(256) VALUE 'Create table'.
          05 WS-Btn-CreateTable-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-CreateTable-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-NewPet.
          05 WS-Btn-NewPet-TEXT       PIC X(256) VALUE 'New pet'.
          05 WS-Btn-NewPet-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-NewPet-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Reload.
          05 WS-Btn-Reload-TEXT       PIC X(256) VALUE 'Reload'.
          05 WS-Btn-Reload-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Reload-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-ClearFilters.
          05 WS-Btn-ClearFilters-TEXT       PIC X(256) VALUE 'Clear filters'.
          05 WS-Btn-ClearFilters-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-ClearFilters-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-DdbState.
          05 WS-Lbl-DdbState-TEXT       PIC X(256) VALUE 'Press Reload to read the table.'.
          05 WS-Lbl-DdbState-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-DdbState-ENABLED    PIC 9      VALUE 1.

       01 WS-Dg-Pets.
          05 WS-Dg-Pets-TEXT       PIC X(256) VALUE 'Dg-Pets'.
          05 WS-Dg-Pets-VISIBLE    PIC 9      VALUE 1.
          05 WS-Dg-Pets-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-S3Head.
          05 WS-Lbl-S3Head-TEXT       PIC X(256) VALUE 'Buckets of the account'.
          05 WS-Lbl-S3Head-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-S3Head-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Bucket.
          05 WS-Cmb-Bucket-TEXT       PIC X(256) VALUE 'Cmb-Bucket'.
          05 WS-Cmb-Bucket-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Bucket-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Bucket-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Btn-S3Reload.
          05 WS-Btn-S3Reload-TEXT       PIC X(256) VALUE 'Reload'.
          05 WS-Btn-S3Reload-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-S3Reload-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-S3State.
          05 WS-Lbl-S3State-TEXT       PIC X(256) VALUE 'Press Reload to read the buckets.'.
          05 WS-Lbl-S3State-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-S3State-ENABLED    PIC 9      VALUE 1.

       01 WS-Dg-S3.
          05 WS-Dg-S3-TEXT       PIC X(256) VALUE 'Dg-S3'.
          05 WS-Dg-S3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Dg-S3-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-LamHead.
          05 WS-Lbl-LamHead-TEXT       PIC X(256) VALUE 'Temporary links, made by a Lambda function'.
          05 WS-Lbl-LamHead-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-LamHead-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-LamText.
          05 WS-Lbl-LamText-TEXT       PIC X(544) VALUE 'PowerCloud never signs a download link itself. The function powercloud-presign does: given the key of one photo, it answers a link to that object that works for 15 minutes, signed with a role that can only read the photo bucket. Use it to hand a photo to someone outside the application, in a browser or an e-mail. The application itself reads its photos through AWS''s MCP server, as it makes every other AWS call; it never fetches the bytes behind a link. AwsLambda.Invoke needs AllowWrite on, because a function can do whatever its code says.'.
          05 WS-Lbl-LamText-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-LamText-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-LamSrc.
          05 WS-Lbl-LamSrc-TEXT       PIC X(256) VALUE 'The function, as the stack deploys it'.
          05 WS-Lbl-LamSrc-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-LamSrc-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-LamSource.
          05 WS-Txt-LamSource-TEXT       PIC X(2048) VALUE 'import os import boto3 from botocore.config import Config  s3 = boto3.client("s3", config=Config(signature_version="s3v4"))  def handler(event, context):     # event: {"key": "photos/<id>.jpg", "expires": 900}     event = event or {}     key = str(event.get("key", ""))     if not key.startswith("photos/") or ".." in key:         return {"error": "the key must be an object under photos/"}     expires = int(event.get("expires") or os.environ.get("EXPIRES", "900"))     url = s3.generate_presigned_url(         "get_object",         Params={"Bucket": os.environ["BUCKET"], "Key": key},         ExpiresIn=max(60, min(expires, 3600)),     )     return {"url": url, "expires": expires}'.
          05 WS-Txt-LamSource-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-LamSource-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-LamSource-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Lbl-LamKey.
          05 WS-Lbl-LamKey-TEXT       PIC X(256) VALUE 'Object key'.
          05 WS-Lbl-LamKey-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-LamKey-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-LamKey.
          05 WS-Txt-LamKey-TEXT       PIC X(256) VALUE 'photos/'.
          05 WS-Txt-LamKey-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-LamKey-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-LamKey-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Btn-LamTry.
          05 WS-Btn-LamTry-TEXT       PIC X(256) VALUE 'Try it'.
          05 WS-Btn-LamTry-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-LamTry-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-LamResult.
          05 WS-Lbl-LamResult-TEXT       PIC X(256) VALUE 'Nothing asked yet.'.
          05 WS-Lbl-LamResult-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-LamResult-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-PollyHead.
          05 WS-Lbl-PollyHead-TEXT       PIC X(256) VALUE 'Text to speech'.
          05 WS-Lbl-PollyHead-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-PollyHead-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Lang.
          05 WS-Lbl-Lang-TEXT       PIC X(256) VALUE 'Language and voice'.
          05 WS-Lbl-Lang-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Lang-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Lang.
          05 WS-Cmb-Lang-TEXT       PIC X(256) VALUE 'Cmb-Lang'.
          05 WS-Cmb-Lang-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Lang-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Lang-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Txt-PollyText.
          05 WS-Txt-PollyText-TEXT       PIC X(2048) VALUE 'Hello from PowerCloud. This sentence was written in RustCOBOL, spoken by Amazon Polly and played by the same program.'.
          05 WS-Txt-PollyText-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-PollyText-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-PollyText-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Btn-Speak.
          05 WS-Btn-Speak-TEXT       PIC X(256) VALUE 'Speak'.
          05 WS-Btn-Speak-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Speak-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-PollyState.
          05 WS-Lbl-PollyState-TEXT       PIC X(256) VALUE 'Type a text and press Speak.'.
          05 WS-Lbl-PollyState-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-PollyState-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Play.
          05 WS-Btn-Play-TEXT       PIC X(256) VALUE ''.
          05 WS-Btn-Play-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Play-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Pause.
          05 WS-Btn-Pause-TEXT       PIC X(256) VALUE ''.
          05 WS-Btn-Pause-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Pause-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Stop.
          05 WS-Btn-Stop-TEXT       PIC X(256) VALUE ''.
          05 WS-Btn-Stop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Stop-ENABLED    PIC 9      VALUE 1.

       01 WS-Pgb-Player.
          05 WS-Pgb-Player-TEXT       PIC X(256) VALUE 'Pgb-Player'.
          05 WS-Pgb-Player-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pgb-Player-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Time.
          05 WS-Lbl-Time-TEXT       PIC X(256) VALUE '00:00 / 00:00'.
          05 WS-Lbl-Time-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Time-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Player.
          05 WS-Tmr-Player-TEXT       PIC X(256) VALUE 'Tmr-Player'.
          05 WS-Tmr-Player-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Player-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Ec2Head.
          05 WS-Lbl-Ec2Head-TEXT       PIC X(256) VALUE 'Servers'.
          05 WS-Lbl-Ec2Head-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Ec2Head-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Ec2Region.
          05 WS-Cmb-Ec2Region-TEXT       PIC X(256) VALUE 'Cmb-Ec2Region'.
          05 WS-Cmb-Ec2Region-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Ec2Region-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Ec2Region-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Btn-Ec2Reload.
          05 WS-Btn-Ec2Reload-TEXT       PIC X(256) VALUE 'Reload'.
          05 WS-Btn-Ec2Reload-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Ec2Reload-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Ec2State.
          05 WS-Lbl-Ec2State-TEXT       PIC X(256) VALUE 'Pick a region or press Reload.'.
          05 WS-Lbl-Ec2State-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Ec2State-ENABLED    PIC 9      VALUE 1.

       01 WS-Dg-Ec2.
          05 WS-Dg-Ec2-TEXT       PIC X(256) VALUE 'Dg-Ec2'.
          05 WS-Dg-Ec2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Dg-Ec2-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Ec2.
          05 WS-Tmr-Ec2-TEXT       PIC X(256) VALUE 'Tmr-Ec2'.
          05 WS-Tmr-Ec2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Ec2-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-RekHead.
          05 WS-Lbl-RekHead-TEXT       PIC X(256) VALUE 'Who is in the picture?'.
          05 WS-Lbl-RekHead-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-RekHead-ENABLED    PIC 9      VALUE 1.

       01 WS-Drp-Rek.
          05 WS-Drp-Rek-TEXT       PIC X(256) VALUE 'Drp-Rek'.
          05 WS-Drp-Rek-VISIBLE    PIC 9      VALUE 1.
          05 WS-Drp-Rek-ENABLED    PIC 9      VALUE 1.
          05 WS-Drp-Rek-FILE-COUNT PIC S9(4) VALUE 0.
          05 WS-Drp-Rek-FILE-PATH  PIC X(1024) OCCURS 20 TIMES
                                      VALUE SPACES.

       01 WS-Btn-Recognize.
          05 WS-Btn-Recognize-TEXT       PIC X(256) VALUE 'Recognize'.
          05 WS-Btn-Recognize-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Recognize-ENABLED    PIC 9      VALUE 1.

       01 WS-Pic-Rek.
          05 WS-Pic-Rek-TEXT       PIC X(256) VALUE 'Pic-Rek'.
          05 WS-Pic-Rek-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pic-Rek-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Box1.
          05 WS-Shp-Box1-TEXT       PIC X(256) VALUE 'Shp-Box1'.
          05 WS-Shp-Box1-VISIBLE    PIC 9      VALUE 0.
          05 WS-Shp-Box1-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Box2.
          05 WS-Shp-Box2-TEXT       PIC X(256) VALUE 'Shp-Box2'.
          05 WS-Shp-Box2-VISIBLE    PIC 9      VALUE 0.
          05 WS-Shp-Box2-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Box3.
          05 WS-Shp-Box3-TEXT       PIC X(256) VALUE 'Shp-Box3'.
          05 WS-Shp-Box3-VISIBLE    PIC 9      VALUE 0.
          05 WS-Shp-Box3-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Box4.
          05 WS-Shp-Box4-TEXT       PIC X(256) VALUE 'Shp-Box4'.
          05 WS-Shp-Box4-VISIBLE    PIC 9      VALUE 0.
          05 WS-Shp-Box4-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Box5.
          05 WS-Shp-Box5-TEXT       PIC X(256) VALUE 'Shp-Box5'.
          05 WS-Shp-Box5-VISIBLE    PIC 9      VALUE 0.
          05 WS-Shp-Box5-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Box6.
          05 WS-Shp-Box6-TEXT       PIC X(256) VALUE 'Shp-Box6'.
          05 WS-Shp-Box6-VISIBLE    PIC 9      VALUE 0.
          05 WS-Shp-Box6-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-RekDetails.
          05 WS-Lbl-RekDetails-TEXT       PIC X(256) VALUE 'Drop a photo, then press Recognize.'.
          05 WS-Lbl-RekDetails-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-RekDetails-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "MAIN-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "MAIN-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Close"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLOSE--ONCLICK"
                       END-EVALUATE
                   WHEN "Cmb-Language"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onSelectedIndexChanged"
                               CALL "CMB-LANGUAGE--ONSELECTEDINDEXCHANGED"
                       END-EVALUATE
                   WHEN "Tab-Main"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTabChanged"
                               CALL "TAB-MAIN--ONTABCHANGED"
                       END-EVALUATE
                   WHEN "Tmr-Stack"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-STACK--ONTICK"
                       END-EVALUATE
                   WHEN "Cmb-Region"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onSelectedIndexChanged"
                               CALL "CMB-REGION--ONSELECTEDINDEXCHANGED"
                       END-EVALUATE
                   WHEN "Btn-Connect"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CONNECT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Deploy"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-DEPLOY--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Save"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Lbl-Link"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "LBL-LINK--ONCLICK"
                       END-EVALUATE
                   WHEN "Tmr-Thumbs"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-THUMBS--ONTICK"
                       END-EVALUATE
                   WHEN "Btn-CreateTable"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CREATETABLE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-NewPet"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NEWPET--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Reload"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-RELOAD--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-ClearFilters"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLEARFILTERS--ONCLICK"
                       END-EVALUATE
                   WHEN "Dg-Pets"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCellClick"
                               CALL "DG-PETS--ONCELLCLICK"
                       END-EVALUATE
                   WHEN "Cmb-Bucket"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onSelectedIndexChanged"
                               CALL "CMB-BUCKET--ONSELECTEDINDEXCHANGED"
                       END-EVALUATE
                   WHEN "Btn-S3Reload"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-S3RELOAD--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-LamTry"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LAMTRY--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Speak"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SPEAK--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Play"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PLAY--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Pause"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PAUSE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Stop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-STOP--ONCLICK"
                       END-EVALUATE
                   WHEN "Tmr-Player"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-PLAYER--ONTICK"
                       END-EVALUATE
                   WHEN "Cmb-Ec2Region"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onSelectedIndexChanged"
                               CALL "CMB-EC2REGION--ONSELECTEDINDEXCHANGED"
                       END-EVALUATE
                   WHEN "Btn-Ec2Reload"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-EC2RELOAD--ONCLICK"
                       END-EVALUATE
                   WHEN "Dg-Ec2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCellEdited"
                               CALL "DG-EC2--ONCELLEDITED"
                       END-EVALUATE
                   WHEN "Tmr-Ec2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-EC2--ONTICK"
                       END-EVALUATE
                   WHEN "Drp-Rek"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onFilesDropped"
                               CALL "DRP-REK--ONFILESDROPPED"
                       END-EVALUATE
                   WHEN "Btn-Recognize"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-RECOGNIZE--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE Tmr-Stack 'SetInterval' USING BY VALUE 3000
           INVOKE Tmr-Thumbs 'SetInterval' USING BY VALUE 300
           INVOKE Tmr-Player 'SetInterval' USING BY VALUE 250
           INVOKE Tmr-Ec2 'SetInterval' USING BY VALUE 2000
           CONTINUE.

      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Every status line starts with its designed message; the language starts as English.

           SET     WS-CONN-MSG                   TO    M-NOT-CONNECTED
           SET     WS-STK-MSG                    TO    M-NO-STACK
           SET     WS-DDB-MSG                    TO    M-PRESS-RELOAD-TABLE
           SET     WS-S3-MSG                     TO    M-PRESS-RELOAD-BUCKETS
           SET     WS-LAM-MSG                    TO    M-NOTHING-ASKED
           SET     WS-POL-MSG                    TO    M-TYPE-AND-SPEAK
           SET     WS-EC2-MSG                    TO    M-PICK-REGION
           SET     WS-REK-MSG                    TO    M-REK-INITIAL

           *> PC-TEXTS: shows every text of this form in the language WS-LANG-IX names (1, English,
           *> here).
           *> Uses:   WS-LANG-IX = 1 and the status messages set just above.
           *> Result: The texts, tab titles, grid headings, voices and status lines are in English,
           *>         the
           *>         language they were designed in; the property UiLanguage is set for the popups.

           CALL    "PC-TEXTS"

           GOBACK.

       END PROGRAM MAIN-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM MAIN-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLOSE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    me::Close()

           GOBACK.

       END PROGRAM BTN-CLOSE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMB-LANGUAGE--ONSELECTEDINDEXCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The language picked, 1 to 6 in the order of the list: en, pt, es, fr, jp, cn.

           SET     WS-LANG-IX                    TO    CMB-LANGUAGE::GetSelectedIndex()

           *> PC-TEXTS: shows every text of this form in the language WS-LANG-IX names.
           *> Uses:   WS-LANG-IX = the position just picked in CMB-LANGUAGE (1 en, 2 pt, 3 es, 4 fr,
           *>         5 jp, 6 cn) and the text tables in working storage.
           *> Result: Every designed caption, hint, tab title and grid heading, the voices of
           *>         CMB-LANG
           *>         and the status lines are in the new language; the property UiLanguage of this
           *>         form says which one, for the popups it opens.

           CALL    "PC-TEXTS"

           GOBACK.

       END PROGRAM CMB-LANGUAGE--ONSELECTEDINDEXCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TAB-MAIN--ONTABCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> A tab reads its data the first time it is opened, once the keys have been given.

           EVALUATE TAB-MAIN::SelectedTab
               WHEN 2
                   IF WS-PETS-LOADED = "N"

                       *> PC-PETS-LOAD: reads every pet of the table into the grid.
                       *> Uses:   AWS-DDB (table PowerCloudPets, in the region of the Settings tab).
                       *> Result: DG-PETS holds one row per pet; LBL-DDBSTATE says how many, or what
                       *>         went wrong.

                       CALL    "PC-PETS-LOAD"
                   END-IF
               WHEN 3
                   IF CMB-BUCKET::GetCount() = 0

                       *> PC-S3-BUCKETS: asks the account for its buckets and fills the list.
                       *> Uses:   AWS-S3-LIST in the region of the Settings tab.
                       *> Result: CMB-BUCKET holds the bucket names; the first one is picked and
                       *>         listed.

                       CALL    "PC-S3-BUCKETS"
                   END-IF
               WHEN 6
                   IF DG-EC2::GetRowCount() = 0

                       *> PC-EC2-LOAD: lists the servers of the region picked.
                       *> Uses:   AWS-EC2::Connection = the region of the list.
                       *> Result: DG-EC2 holds one row per server; LBL-EC2STATE says how many.

                       CALL    "PC-EC2-LOAD"
                   END-IF
           END-EVALUATE

           GOBACK.

       END PROGRAM TAB-MAIN--ONTABCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-STACK--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-STACK-REFRESH: asks CloudFormation how the stack stands.
           *> Uses:   AWS-MCP (DescribeStacks of the stack named powercloud) and TXT-STACK::Text.
           *> Result: LBL-STACKSTATE and PGB-STACK show the status; on CREATE_COMPLETE the timer
           *>         stops and
           *>         TXT-BUCKET, TXT-FUNCTION, AWS-S3::Bucket and AWS-LAMBDA::FunctionName take the
           *> stack's outputs.

           CALL    "PC-STACK-REFRESH"

           GOBACK.

       END PROGRAM TMR-STACK--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMB-REGION--ONSELECTEDINDEXCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Every control but EC2's own list follows the region picked here.

           SET     AWS-MCP::Connection           TO    CMB-REGION::GetSelected()
           SET     AWS-DDB::Connection           TO    CMB-REGION::GetSelected()
           SET     AWS-S3::Connection            TO    CMB-REGION::GetSelected()
           SET     AWS-S3-LIST::Connection       TO    CMB-REGION::GetSelected()
           SET     AWS-LAMBDA::Connection        TO    CMB-REGION::GetSelected()
           SET     AWS-POLLY::Connection         TO    CMB-REGION::GetSelected()
           SET     AWS-REK::Connection           TO    CMB-REGION::GetSelected()

           GOBACK.

       END PROGRAM CMB-REGION--ONSELECTEDINDEXCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CONNECT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Hand the keys to the runtime: one call per region's connection. They are kept in
           *> memory only, and the boxes are emptied at once. Both boxes empty: nothing is
           *> handed over - keys typed earlier in this run stay, and without any the machine's
           *> own AWS sign-in is used (its default profile, IAM Identity Center, the
           *> environment), as the hint above the boxes says. Any machine, any account.

           SET     WS-STATUS                     TO    SPACES
           IF TXT-KEYID::Text NOT = SPACES OR TXT-SECRET::Text NOT = SPACES
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 6
                   COBOL::"AWS-SET-CREDENTIALS" ( WS-REGION(WS-I) TXT-KEYID::Text TXT-SECRET::Text TXT-TOKEN::Text WS-STATUS )
               END-PERFORM
           END-IF
           SET     TXT-KEYID::Text               TO    SPACES
           SET     TXT-SECRET::Text              TO    SPACES
           SET     TXT-TOKEN::Text               TO    SPACES

           IF WS-STATUS NOT = "OK" AND WS-STATUS NOT = SPACES
               SET     WS-CONN-MSG                   TO    M-KEYS-REFUSED
               SET     WS-CONN-A1                    TO    WS-STATUS::Trim()
               SET     LBL-CONN::Caption             TO    PC-TEXT-NOW(WS-CONN-MSG)::Trim()::Replace( "&1", WS-CONN-A1::Trim() )
               GOBACK
           END-IF

           *> One read-only call proves who the keys belong to.

           SET     WS-CONN-MSG                   TO    M-CHECKING-KEYS
           SET     LBL-CONN::Caption             TO    PC-TEXT-NOW(WS-CONN-MSG)::Trim()
           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "sts" "GetCallerIdentity" "{}" )
           IF AWS-MCP::LastError NOT = SPACES
               SET     WS-CONN-MSG                   TO    M-NOT-CONNECTED-WHY
               SET     WS-CONN-A1                    TO    AWS-MCP::LastError::Trim()
               SET     LBL-CONN::Caption             TO    PC-TEXT-NOW(WS-CONN-MSG)::Trim()::Replace( "&1", WS-CONN-A1::Trim() )
           ELSE
               SET     WS-CONN-MSG                   TO    M-CONNECTED-AS
               SET     WS-CONN-A1                    TO    AWS-MCP::GetField( 1 "Arn" )::Trim()
               SET     LBL-CONN::Caption             TO    PC-TEXT-NOW(WS-CONN-MSG)::Trim()::Replace( "&1", WS-CONN-A1::Trim() )

               *> A stack deployed on an earlier run is found now: without this the photo bucket
               *> and the link function stayed empty until a deployment finished in this session,
               *> and every photo was refused (operator, 2026-10-18).
               *> PC-STACK-OUTPUTS: the stack's outputs into the boxes and the controls.
               *> Uses:   AWS-MCP (DescribeStacks of the stack named in TXT-STACK).
               *> Result: TXT-BUCKET, TXT-FUNCTION, AWS-S3::Bucket and AWS-LAMBDA::FunctionName.

               CALL    "PC-STACK-OUTPUTS"
           END-IF

           GOBACK.

       END PROGRAM BTN-CONNECT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-DEPLOY--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-TEMPLATE-JSON: reads the template file and leaves it as JSON text in WS-TEMPLATE.
           *> Uses:   WS-TEMPLATE-PATH = "cloudformation/powercloud.yaml" (set in working storage).
           *> Result: WS-TEMPLATE = the file as the inside of a JSON string; WS-STATUS = "OK", or
           *>         the file status when it
           *>         could not be read; WS-TPOS, WS-LINE and WS-LINE-LEN are overwritten.

           CALL    "PC-TEMPLATE-JSON"
           IF WS-STATUS NOT = "OK"
               SET     WS-STK-MSG                    TO    M-TEMPLATE-UNREADABLE
               SET     WS-STK-A1                     TO    WS-STATUS::Trim()
               SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()::Replace( "&1", WS-STK-A1::Trim() )
               GOBACK
           END-IF

           *> A role an administrator made for the function (the powercloud-deployer stack, see
           *> the README) is handed to the stack: keys allowed to deploy, but never to create an
           *> IAM role, can deploy it then. Without that stack the stack makes its own role.

           SET     WS-FN-ROLE                    TO    SPACES
           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "cloudformation" "DescribeStacks" '{"StackName":"powercloud-deployer"}' )
           IF AWS-MCP::LastError = SPACES
               SET     WS-OUTPUTS                    TO    AWS-MCP::GetField( 1 "Outputs" )::Replace( '": "', '":"' )
               SET     WS-FN-ROLE                    TO    WS-OUTPUTS::Split( '"OutputKey":"FunctionRoleArn","OutputValue":"' )(2)::Split( '"' )(1)
           END-IF

           STRING '{"StackName":"' TXT-STACK::Text::Trim() '","Capabilities":["CAPABILITY_IAM"],'
                  '"Parameters":[{"ParameterKey":"FunctionName","ParameterValue":"' TXT-FUNCTION::Text::Trim() '"},'
                  '{"ParameterKey":"FunctionRoleArn","ParameterValue":"' WS-FN-ROLE::Trim() '"}],'
                  '"TemplateBody":"' WS-TEMPLATE::Trim() '"}'
                  DELIMITED BY SIZE INTO WS-PARAMS

           SET     WS-STK-MSG                    TO    M-CREATING-STACK
           SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()
           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "cloudformation" "CreateStack" WS-PARAMS::Trim() )
           IF AWS-MCP::LastError NOT = SPACES
               SET     WS-STK-MSG                    TO    M-CF-REFUSED
               SET     WS-STK-A1                     TO    AWS-MCP::LastError::Trim()

               *> Keys made to run the demo are not allowed to deploy it (creating the stack
               *> creates an IAM role and a policy): say what to do. The whole refusal is in
               *> the Output panel.

               IF WS-STK-A1(1:12) = "AccessDenied"
                   SET     WS-STK-MSG                    TO    M-CF-NO-RIGHTS
               END-IF
               SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()::Replace( "&1", WS-STK-A1::Trim() )
           ELSE
               SET     PGB-STACK::Value              TO    5
               SET     TMR-STACK::Enabled            TO    TRUE
           END-IF

           GOBACK.

       END PROGRAM BTN-DEPLOY--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-SAVE-TEMPLATE: asks where to save, then copies the template file there.
           *> Uses:   WS-TEMPLATE-PATH = the template that ships with the example.
           *> Result: The file the user chose holds a copy; WS-SAVE-PATH = its path, or SPACES when
           *>         the dialog was
           *>         cancelled; LBL-STACKSTATE says what happened; WS-FS and WS-FS-OUT are
           *> overwritten.

           CALL    "PC-SAVE-TEMPLATE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-LINK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-SAVE-TEMPLATE: asks where to save, then copies the template file there.
           *> Uses:   WS-TEMPLATE-PATH = the template that ships with the example.
           *> Result: The file the user chose holds a copy; WS-SAVE-PATH = its path, or SPACES when
           *>         the dialog was
           *>         cancelled; LBL-STACKSTATE says what happened; WS-FS and WS-FS-OUT are
           *> overwritten.

           CALL    "PC-SAVE-TEMPLATE"

           GOBACK.

       END PROGRAM LBL-LINK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-THUMBS--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> One thumbnail per tick, so the grid shows its rows first and the pictures arrive
           *> after.

           IF WS-THUMB-NEXT > WS-PET-COUNT
               SET     TMR-THUMBS::Enabled           TO    FALSE
               GOBACK
           END-IF

           *> PC-THUMB-ENSURE: makes sure the 40 x 40 thumbnail of one pet is in the local cache.
           *> Uses:   WS-THUMB-NEXT = the row of WS-PET-TABLE to look at (its id and photo key).
           *> Result: data/cache/<id>-40.png exists when the photo could be fetched; WS-OK = "OK" or
           *>         why not.

           CALL    "PC-THUMB-ENSURE"
           ADD 1 TO WS-THUMB-NEXT

           GOBACK.

       END PROGRAM TMR-THUMBS--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CREATETABLE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Describe the table first: it is created only when AWS says it does not exist.

           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "dynamodb" "DescribeTable" '{"TableName":"PowerCloudPets"}' )
           IF AWS-MCP::LastError = SPACES
               SET     WS-DDB-MSG                    TO    M-TABLE-EXISTS
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()
               GOBACK
           END-IF
           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "dynamodb" "CreateTable" '{"TableName":"PowerCloudPets","AttributeDefinitions":[{"AttributeName":"id","AttributeType":"S"}],"KeySchema":[{"AttributeName":"id","KeyType":"HASH"}],"BillingMode":"PAY_PER_REQUEST"}' )
           IF AWS-MCP::LastError = SPACES
               SET     WS-DDB-MSG                    TO    M-TABLE-CREATED
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()
           ELSE
               SET     WS-DDB-MSG                    TO    M-TABLE-NOT-CREATED
               SET     WS-DDB-A1                     TO    AWS-MCP::LastError::Trim()
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )
           END-IF

           GOBACK.

       END PROGRAM BTN-CREATETABLE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEWPET--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-PET-NEW: asks for a new pet in the popup and stores it when it is saved.
           *> Uses:   No data item is read; the popup form PET-FORM does the asking.
           *> Result: A pet is stored in DynamoDB (and its photo in S3) when the popup returned
           *>         PetSaved = "Y";
           *>         the grid is reloaded; LBL-DDBSTATE says what happened.

           CALL    "PC-PET-NEW"

           GOBACK.

       END PROGRAM BTN-NEWPET--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-RELOAD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-PETS-LOAD: reads every pet of the table into the grid.
           *> Uses:   AWS-DDB (table PowerCloudPets, in the region of the Settings tab).
           *> Result: DG-PETS holds one row per pet; WS-PET-TABLE holds each pet's id and photo key;
           *>         the thumbnail timer starts; LBL-DDBSTATE says how many pets, or what went
           *> wrong.

           CALL    "PC-PETS-LOAD"

           GOBACK.

       END PROGRAM BTN-RELOAD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLEARFILTERS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           DG-PETS::ClearFilters()

           GOBACK.

       END PROGRAM BTN-CLEARFILTERS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DG-PETS--ONCELLCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The three icon columns of a row: View (6), Edit (7) and Delete (8).

           SET     WS-ID                         TO    DG-PETS::GetCellValue( DG-PETS::ClickedRow 1 )
           EVALUATE DG-PETS::ClickedColumn
               WHEN 6

                   *> PC-PET-VIEW: shows one pet, read-only, with its 400 x 400 photo.
                   *> Uses:   WS-ID = the id of the clicked row (grid column 1).
                   *> Result: The popup VIEW-FORM was shown and closed; no data item changes.

                   CALL    "PC-PET-VIEW"
               WHEN 7

                   *> PC-PET-EDIT: edits one pet in the popup and stores the change.
                   *> Uses:   WS-ID = the id of the clicked row (grid column 1).
                   *> Result: The pet is stored again when the popup returned PetSaved = "Y"; the
                   *>         grid is reloaded.

                   CALL    "PC-PET-EDIT"
               WHEN 8

                   *> PC-PET-DELETE: asks to confirm, then removes the item and its photo.
                   *> Uses:   WS-ID = the id of the clicked row (grid column 1).
                   *> Result: The item and the object photos/<id>.jpg are deleted after a Yes; the
                   *>         grid is reloaded.

                   CALL    "PC-PET-DELETE"
           END-EVALUATE

           GOBACK.

       END PROGRAM DG-PETS--ONCELLCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMB-BUCKET--ONSELECTEDINDEXCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-S3-LIST: lists the objects of the bucket picked.
           *> Uses:   CMB-BUCKET::GetSelected() = the bucket picked.
           *> Result: DG-S3 holds one row per object; LBL-S3STATE says how many.

           CALL    "PC-S3-LIST"

           GOBACK.

       END PROGRAM CMB-BUCKET--ONSELECTEDINDEXCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-S3RELOAD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-S3-BUCKETS: asks the account for its buckets and fills the list.
           *> Uses:   AWS-S3-LIST in the region of the Settings tab.
           *> Result: CMB-BUCKET holds the bucket names; the first one is picked and listed.

           CALL    "PC-S3-BUCKETS"

           GOBACK.

       END PROGRAM BTN-S3RELOAD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LAMTRY--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-JSON                       TO    SPACES
           STRING '{"key":"' TXT-LAMKEY::Text::Trim() '"}' INTO WS-JSON
           SET     WS-OUT                        TO    AWS-LAMBDA::Invoke( "" WS-JSON::Trim() )
           IF AWS-LAMBDA::LastError NOT = SPACES
               SET     WS-LAM-MSG                    TO    M-FUNCTION-NOT-CALLED
               SET     WS-LAM-A1                     TO    AWS-LAMBDA::LastError::Trim()
               SET     LBL-LAMRESULT::Caption        TO    PC-TEXT-NOW(WS-LAM-MSG)::Trim()::Replace( "&1", WS-LAM-A1::Trim() )
               GOBACK
           END-IF
           SET     WS-URL                        TO    WS-OUT::Replace( '": "', '":"' )::Split( '"url":"' )(2)::Split( '"' )(1)
           IF WS-URL = SPACES
               SET     WS-LAM-MSG                    TO    M-FUNCTION-ANSWERED
               SET     WS-LAM-A1                     TO    WS-OUT::Trim()
               SET     LBL-LAMRESULT::Caption        TO    PC-TEXT-NOW(WS-LAM-MSG)::Trim()::Replace( "&1", WS-LAM-A1::Trim() )
           ELSE

               *> Only the part before the question mark is shown: the signature after it is a
               *> secret for 15 minutes.

               SET     WS-LAM-MSG                    TO    M-LINK-RESULT
               SET     WS-LAM-A1                     TO    WS-URL::Split( "?" )(1)
               SET     LBL-LAMRESULT::Caption        TO    PC-TEXT-NOW(WS-LAM-MSG)::Trim()::Replace( "&1", WS-LAM-A1::Trim() )
           END-IF

           GOBACK.

       END PROGRAM BTN-LAMTRY--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SPEAK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-VOICE-IX                   TO    CMB-LANG::GetSelectedIndex()
           SET     WS-POL-MSG                    TO    M-ASKING-POLLY
           SET     LBL-POLLYSTATE::Caption       TO    PC-TEXT-NOW(WS-POL-MSG)::Trim()
           SET     WS-OUT                        TO    AWS-POLLY::Synthesize( TXT-POLLYTEXT::Text WS-VOICE(WS-VOICE-IX)::Trim() "mp3" "data/cache/speech.mp3" )
           IF AWS-POLLY::LastError NOT = SPACES
               SET     WS-POL-MSG                    TO    M-POLLY-FAILED
               SET     WS-POL-A1                     TO    AWS-POLLY::LastError::Trim()
               SET     LBL-POLLYSTATE::Caption       TO    PC-TEXT-NOW(WS-POL-MSG)::Trim()::Replace( "&1", WS-POL-A1::Trim() )
               GOBACK
           END-IF
           SET     WS-POL-MSG                    TO    M-POLLY-READY
           SET     WS-POL-A1                     TO    AWS-POLLY::Characters
           SET     LBL-POLLYSTATE::Caption       TO    PC-TEXT-NOW(WS-POL-MSG)::Trim()::Replace( "&1", WS-POL-A1::Trim() )
           COBOL::"AUDIO-PLAY" ( "data/cache/speech.mp3" WS-STATUS )
           SET     TMR-PLAYER::Enabled           TO    TRUE

           GOBACK.

       END PROGRAM BTN-SPEAK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PLAY--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           COBOL::"AUDIO-STATE" ( WS-AUD-STATE WS-POS-MS WS-DUR-MS )
           IF WS-AUD-STATE = "PAUSED"
               COBOL::"AUDIO-RESUME" ( WS-STATUS )
           ELSE
               COBOL::"AUDIO-PLAY" ( "data/cache/speech.mp3" WS-STATUS )
           END-IF
           SET     TMR-PLAYER::Enabled           TO    TRUE

           GOBACK.

       END PROGRAM BTN-PLAY--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PAUSE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           COBOL::"AUDIO-PAUSE" ( WS-STATUS )

           GOBACK.

       END PROGRAM BTN-PAUSE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-STOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           COBOL::"AUDIO-STOP" ( WS-STATUS )
           SET     PGB-PLAYER::Value             TO    0
           SET     LBL-TIME::Caption             TO    "00:00 / 00:00"

           GOBACK.

       END PROGRAM BTN-STOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-PLAYER--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           COBOL::"AUDIO-STATE" ( WS-AUD-STATE WS-POS-MS WS-DUR-MS )
           IF WS-DUR-MS > 0
               COMPUTE WS-PCT = WS-POS-MS * 100 / WS-DUR-MS
               SET     PGB-PLAYER::Value             TO    WS-PCT
           END-IF
           COMPUTE WS-SEC = WS-POS-MS / 1000
           DIVIDE WS-SEC BY 60 GIVING WS-MIN REMAINDER WS-SECS
           COMPUTE WS-DUR-SEC = WS-DUR-MS / 1000
           DIVIDE WS-DUR-SEC BY 60 GIVING WS-DMIN REMAINDER WS-DSECS
           STRING WS-MIN ":" WS-SECS " / " WS-DMIN ":" WS-DSECS DELIMITED BY SIZE INTO LBL-TIME::Caption
           IF WS-AUD-STATE = "ENDED" OR WS-AUD-STATE = "IDLE" OR WS-AUD-STATE = "ERROR"
               SET     TMR-PLAYER::Enabled           TO    FALSE
           END-IF

           GOBACK.

       END PROGRAM TMR-PLAYER--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMB-EC2REGION--ONSELECTEDINDEXCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     AWS-EC2::Connection           TO    CMB-EC2REGION::GetSelected()

           *> PC-EC2-LOAD: lists the servers of the region picked.
           *> Uses:   AWS-EC2::Connection = the region just picked.
           *> Result: DG-EC2 holds one row per server; LBL-EC2STATE says how many.

           CALL    "PC-EC2-LOAD"

           GOBACK.

       END PROGRAM CMB-EC2REGION--ONSELECTEDINDEXCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-EC2RELOAD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-EC2-LOAD: lists the servers of the region picked.
           *> Uses:   AWS-EC2::Connection = the region of the list.
           *> Result: DG-EC2 holds one row per server; LBL-EC2STATE says how many.

           CALL    "PC-EC2-LOAD"

           GOBACK.

       END PROGRAM BTN-EC2RELOAD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DG-EC2--ONCELLEDITED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Only the On / off column (7) is meant to change; any other edit is taken back.

           IF DG-EC2::EditedColumn NOT = 7
               DG-EC2::SetCellValue( DG-EC2::EditedRow DG-EC2::EditedColumn DG-EC2::PreviousValue )
               GOBACK
           END-IF
           SET     WS-EC2-ID                     TO    DG-EC2::GetCellValue( DG-EC2::EditedRow 1 )
           IF DG-EC2::EditedValue = "1"
               SET     WS-TARGET                     TO    "running"
               SET     WS-OUT                        TO    AWS-EC2::Start( WS-EC2-ID::Trim() )
           ELSE
               SET     WS-TARGET                     TO    "stopped"
               SET     WS-OUT                        TO    AWS-EC2::Stop( WS-EC2-ID::Trim() )
           END-IF
           IF AWS-EC2::LastError NOT = SPACES
               SET     WS-EC2-MSG                    TO    M-AWS-REFUSED
               SET     WS-EC2-A1                     TO    AWS-EC2::LastError::Trim()
               SET     LBL-EC2STATE::Caption         TO    PC-TEXT-NOW(WS-EC2-MSG)::Trim()::Replace( "&1", WS-EC2-A1::Trim() )

               *> PC-EC2-LOAD: lists the servers again, which puts the switch back where the server
               *> really is.
               *> Uses:   AWS-EC2::Connection = the region of the list.
               *> Result: DG-EC2 shows the real state of every server.

               CALL    "PC-EC2-LOAD"
               GOBACK
           END-IF

           *> The Working window is a separate window that stays on top while the server changes
           *> state; the
           *> list is disabled meanwhile and the timer asks AWS every two seconds.

           IF WS-TARGET = "running"
               SET     WS-WORKING-TEXT               TO    T-WAIT-RUNNING::Trim()::Replace( "&1", WS-EC2-ID::Trim() )
           ELSE
               SET     WS-WORKING-TEXT               TO    T-WAIT-STOPPED::Trim()::Replace( "&1", WS-EC2-ID::Trim() )
           END-IF
           INVOKE    ME::SetProperty( "WorkingText", WS-WORKING-TEXT::Trim() )
           INVOKE    ME::OpenFormAsync( "WORKING-FORM" ) RETURNING WS-WORK
           SET     DG-EC2::Enabled               TO    FALSE
           SET     TMR-EC2::Enabled              TO    TRUE

           GOBACK.

       END PROGRAM DG-EC2--ONCELLEDITED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-EC2--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-OUT                        TO    AWS-EC2::Describe( WS-EC2-ID::Trim() )
           IF AWS-EC2::GetField( 1 "State" ) = WS-TARGET::Trim()
               SET     TMR-EC2::Enabled              TO    FALSE
               INVOKE    WS-WORK::Close()
               SET     DG-EC2::Enabled               TO    TRUE

               *> PC-EC2-LOAD: lists the servers again with the state AWS reports now.
               *> Uses:   AWS-EC2::Connection = the region of the list.
               *> Result: DG-EC2 shows the new state; LBL-EC2STATE says how many servers.

               CALL    "PC-EC2-LOAD"
           END-IF

           GOBACK.

       END PROGRAM TMR-EC2--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DRP-REK--ONFILESDROPPED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-REK-FILE                   TO    DRP-REK::DroppedFiles
           SET     PIC-REK::ImagePath            TO    WS-REK-FILE::Trim()
           SET     WS-REK-LIST                   TO    "N"
           SET     WS-REK-MSG                    TO    M-REK-PRESS
           SET     LBL-REKDETAILS::Caption       TO    PC-TEXT-NOW(WS-REK-MSG)::Trim()
           PERFORM VARYING WS-N FROM 1 BY 1 UNTIL WS-N > 6
               EVALUATE WS-N
                   WHEN 1 SET SHP-BOX1::Visible TO FALSE
                   WHEN 2 SET SHP-BOX2::Visible TO FALSE
                   WHEN 3 SET SHP-BOX3::Visible TO FALSE
                   WHEN 4 SET SHP-BOX4::Visible TO FALSE
                   WHEN 5 SET SHP-BOX5::Visible TO FALSE
                   WHEN 6 SET SHP-BOX6::Visible TO FALSE
               END-EVALUATE
           END-PERFORM

           GOBACK.

       END PROGRAM DRP-REK--ONFILESDROPPED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-RECOGNIZE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-REK-RUN: sends the picture to Rekognition and draws a box round every celebrity
           *> found.
           *> Uses:   WS-REK-FILE = the picture dropped (an absolute path).
           *> Result: LBL-REKDETAILS lists the celebrities; SHP-BOX1..6 frame the faces over
           *>         PIC-REK.

           CALL    "PC-REK-RUN"

           GOBACK.

       END PROGRAM BTN-RECOGNIZE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TEMPLATE-JSON IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Reads the CloudFormation template line by line and leaves it in WS-TEMPLATE as the
           *> inside of a JSON string: a quote becomes \" and the end of a line becomes \n.

           SET     WS-STATUS                     TO    "OK"
           SET     WS-TEMPLATE                   TO    SPACES
           SET     WS-TPOS                       TO    1
           OPEN INPUT TEMPLATE-FILE
           IF WS-FS NOT = "00"
               STRING "file status " WS-FS DELIMITED BY SIZE INTO WS-STATUS
               GOBACK
           END-IF
           PERFORM UNTIL WS-FS NOT = "00"
               READ TEMPLATE-FILE
               IF WS-FS = "00"

                   *> The length of the line without the spaces COBOL pads it with; its own
                   *> indentation stays.

                   SET     WS-LINE-LEN                   TO    400
                   PERFORM UNTIL WS-LINE-LEN = 1 OR TEMPLATE-REC( WS-LINE-LEN : 1 ) NOT = SPACE
                       SUBTRACT 1 FROM WS-LINE-LEN
                   END-PERFORM
                   IF WS-LINE-LEN = 1 AND TEMPLATE-REC( 1 : 1 ) = SPACE
                       STRING "\n" DELIMITED BY SIZE INTO WS-TEMPLATE WITH POINTER WS-TPOS
                   ELSE
                       SET     WS-LINE                       TO    TEMPLATE-REC( 1 : WS-LINE-LEN )::Replace( "\", "\\" )::Replace( '"', '\"' )
                       SET     WS-LINE-LEN                   TO    400
                       PERFORM UNTIL WS-LINE-LEN = 1 OR WS-LINE( WS-LINE-LEN : 1 ) NOT = SPACE
                           SUBTRACT 1 FROM WS-LINE-LEN
                       END-PERFORM
                       STRING WS-LINE( 1 : WS-LINE-LEN ) "\n" DELIMITED BY SIZE INTO WS-TEMPLATE WITH POINTER WS-TPOS
                   END-IF
               END-IF
           END-PERFORM
           CLOSE TEMPLATE-FILE

           GOBACK.

       END PROGRAM PC-TEMPLATE-JSON.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE-TEMPLATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Asks where to save the template and copies it there, line by line.

           COBOL::"SAVE-FILE-DIALOG" ( T-SAVE-DIALOG::Trim() "YAML|*.yaml" "powercloud.yaml" "" WS-SAVE-PATH )
           IF WS-SAVE-PATH = SPACES
               GOBACK
           END-IF
           OPEN INPUT TEMPLATE-FILE
           OPEN OUTPUT SAVE-FILE
           IF WS-FS NOT = "00" OR WS-FS-OUT NOT = "00"
               SET     WS-STK-MSG                    TO    M-TEMPLATE-NOT-SAVED
               SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()
               GOBACK
           END-IF
           PERFORM UNTIL WS-FS NOT = "00"
               READ TEMPLATE-FILE
               IF WS-FS = "00"
                   MOVE TEMPLATE-REC TO SAVE-REC
                   WRITE SAVE-REC
               END-IF
           END-PERFORM
           CLOSE TEMPLATE-FILE
           CLOSE SAVE-FILE
           SET     WS-STK-MSG                    TO    M-TEMPLATE-SAVED
           SET     WS-STK-A1                     TO    WS-SAVE-PATH::Trim()
           SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()::Replace( "&1", WS-STK-A1::Trim() )

           GOBACK.

       END PROGRAM PC-SAVE-TEMPLATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-STACK-REFRESH IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Called by TMR-STACK every three seconds while a stack is being created.

           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "cloudformation" "DescribeStacks" '{"StackName":"' & TXT-STACK::Text::Trim() & '"}' )
           IF AWS-MCP::LastError NOT = SPACES
               SET     WS-STK-MSG                    TO    M-STACK-WAIT
               SET     WS-STK-A1                     TO    AWS-MCP::LastError::Trim()
               SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()::Replace( "&1", WS-STK-A1::Trim() )
               GOBACK
           END-IF
           SET     WS-STACK-STATE                TO    AWS-MCP::GetField( 1 "StackStatus" )
           ADD 7 TO PGB-STACK::Value
           SET     WS-STK-MSG                    TO    M-STACK-STATE
           SET     WS-STK-A1                     TO    TXT-STACK::Text::Trim()
           SET     WS-STK-A2                     TO    WS-STACK-STATE::Trim()
           SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()::Replace( "&1", WS-STK-A1::Trim() )::Replace( "&2", WS-STK-A2::Trim() )
           IF WS-STACK-STATE = "CREATE_COMPLETE"
               SET     TMR-STACK::Enabled            TO    FALSE
               SET     PGB-STACK::Value              TO    100

               *> PC-STACK-OUTPUTS: the stack's outputs into the boxes and the controls.
               *> Uses:   AWS-MCP (DescribeStacks of the stack named in TXT-STACK).
               *> Result: TXT-BUCKET, TXT-FUNCTION, AWS-S3::Bucket and AWS-LAMBDA::FunctionName.

               CALL    "PC-STACK-OUTPUTS"
           END-IF
           IF WS-STACK-STATE(1:6) = "ROLLBA" OR WS-STACK-STATE(1:6) = "CREATE" AND WS-STACK-STATE(8:6) = "FAILED"
               SET     TMR-STACK::Enabled            TO    FALSE
           END-IF

           GOBACK.

       END PROGRAM PC-STACK-REFRESH.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-STACK-OUTPUTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The stack's outputs, wherever the stack was deployed - this session or an earlier one:
           *> the photo bucket into TXT-BUCKET and AWS-S3, the link function into TXT-FUNCTION and
           *> AWS-LAMBDA. Called when a deployment completes and when the keys are connected; a
           *> stack that is not there, or not finished, leaves the boxes as they are, so a name
           *> typed in them is kept.

           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "cloudformation" "DescribeStacks" '{"StackName":"' & TXT-STACK::Text::Trim() & '"}' )
           IF AWS-MCP::LastError NOT = SPACES
               GOBACK
           END-IF
           SET     WS-STACK-STATE                TO    AWS-MCP::GetField( 1 "StackStatus" )
           IF WS-STACK-STATE NOT = "CREATE_COMPLETE" AND WS-STACK-STATE NOT = "UPDATE_COMPLETE"
               GOBACK
           END-IF
           SET     WS-OUTPUTS                    TO    AWS-MCP::GetField( 1 "Outputs" )::Replace( '": "', '":"' )
           SET     TXT-BUCKET::Text              TO    WS-OUTPUTS::Split( '"OutputKey":"BucketName","OutputValue":"' )(2)::Split( '"' )(1)
           SET     TXT-FUNCTION::Text            TO    WS-OUTPUTS::Split( '"OutputKey":"FunctionName","OutputValue":"' )(2)::Split( '"' )(1)
           SET     AWS-S3::Bucket                TO    TXT-BUCKET::Text
           SET     AWS-LAMBDA::FunctionName      TO    TXT-FUNCTION::Text

           GOBACK.

       END PROGRAM PC-STACK-OUTPUTS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-BUCKET-ENSURE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The photo bucket, found when it is needed: the name the Settings tab shows (typed
           *> there, or filled in from the stack), else the stack's outputs, asked for now - so a
           *> run that never pressed Connect, on any machine, still finds it.

           IF TXT-BUCKET::Text NOT = SPACES
               SET     AWS-S3::Bucket                TO    TXT-BUCKET::Text
           END-IF
           IF AWS-S3::Bucket = SPACES

               *> PC-STACK-OUTPUTS: the stack's outputs into the boxes and the controls.
               *> Uses:   AWS-MCP (DescribeStacks of the stack named in TXT-STACK).
               *> Result: TXT-BUCKET, TXT-FUNCTION, AWS-S3::Bucket and AWS-LAMBDA::FunctionName.

               CALL    "PC-STACK-OUTPUTS"
           END-IF

           GOBACK.

       END PROGRAM PC-BUCKET-ENSURE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PETS-LOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-OUT                        TO    AWS-DDB::Scan( 200 )
           IF AWS-DDB::LastError NOT = SPACES
               SET     WS-DDB-MSG                    TO    M-TABLE-NOT-READ
               SET     WS-DDB-A1                     TO    AWS-DDB::LastError::Trim()
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )
               GOBACK
           END-IF
           DG-PETS::ClearRows()
           SET     WS-PET-COUNT                  TO    AWS-DDB::RowCount
           PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > WS-PET-COUNT
               SET     WS-PET-ID(WS-ROW)             TO    AWS-DDB::GetField( WS-ROW "id" )
               SET     WS-PET-KEY(WS-ROW)            TO    AWS-DDB::GetField( WS-ROW "photo" )
               SET     WS-ROWTXT                     TO    SPACES
               STRING WS-PET-ID(WS-ROW)::Trim() X"09" "data/cache/" WS-PET-ID(WS-ROW)::Trim() "-40.png" X"09"
                      AWS-DDB::GetField( WS-ROW "name" ) X"09" AWS-DDB::GetField( WS-ROW "race" ) X"09"
                      AWS-DDB::GetField( WS-ROW "age" ) X"09" "icon:eye" X"09" "icon:pencil" X"09" "icon:trash"
                      INTO WS-ROWTXT
               DG-PETS::AddRow( WS-ROWTXT::Trim() )
           END-PERFORM
           SET     WS-PETS-LOADED                TO    "Y"
           SET     WS-THUMB-NEXT                 TO    1
           SET     TMR-THUMBS::Enabled           TO    TRUE
           SET     WS-NUM                        TO    WS-PET-COUNT
           SET     WS-DDB-MSG                    TO    M-PETS-COUNT
           SET     WS-DDB-A1                     TO    WS-NUM::Trim()
           SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )

           GOBACK.

       END PROGRAM PC-PETS-LOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-THUMB-ENSURE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The thumbnail and the 400 x 400 photo of the pet in row WS-THUMB-NEXT of WS-PET-TABLE.

           SET     WS-ID                         TO    WS-PET-ID(WS-THUMB-NEXT)
           SET     WS-PHOTO-KEY                  TO    WS-PET-KEY(WS-THUMB-NEXT)
           STRING "data/cache/" WS-ID::Trim() "-40.png" INTO WS-THUMB
           COBOL::"IMAGE-INFO" ( WS-THUMB WS-W WS-H WS-FMT WS-OK )
           IF WS-OK = "OK" OR WS-PHOTO-KEY = SPACES
               GOBACK
           END-IF

           *> PC-PHOTO-DOWNLOAD: brings the 400 x 400 photo of the pet WS-ID into the local cache.
           *> Uses:   WS-ID = the pet, WS-PHOTO-KEY = its object key in the bucket
           *>         (photos/<id>.jpg).
           *> Result: WS-LOCAL = the cache path; WS-OK = "OK" when the file is there, else the
           *>         reason.

           CALL    "PC-PHOTO-DOWNLOAD"
           IF WS-OK = "OK"
               COBOL::"IMAGE-FIT" ( WS-LOCAL WS-THUMB 40 40 WS-OK )
           END-IF

           GOBACK.

       END PROGRAM PC-THUMB-ENSURE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PHOTO-DOWNLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           STRING "data/cache/" WS-ID::Trim() "-400.jpg" INTO WS-LOCAL
           COBOL::"IMAGE-INFO" ( WS-LOCAL WS-W WS-H WS-FMT WS-OK )
           IF WS-OK = "OK"
               GOBACK
           END-IF

           *> The photo is read from the bucket through AWS's MCP server, as every AWS call of this
           *> application is - never from a link over plain HTTPS (operator, 2026-10-18).
           *> GetObject saves the object's bytes into the cache file.
           *> PC-BUCKET-ENSURE: the photo bucket into AWS-S3::Bucket - typed, or from the stack.
           *> Result: AWS-S3::Bucket, SPACES when none is known.

           CALL    "PC-BUCKET-ENSURE"
           IF AWS-S3::Bucket = SPACES
               SET     WS-OK                         TO    "NOBUCKET"
               GOBACK
           END-IF
           SET     WS-OUT                        TO    AWS-S3::GetObject( WS-PHOTO-KEY::Trim() WS-LOCAL::Trim() )
           IF AWS-S3::LastError NOT = SPACES
               SET     WS-OK                         TO    "S3"
               GOBACK
           END-IF
           SET     WS-OK                         TO    "OK"

           GOBACK.

       END PROGRAM PC-PHOTO-DOWNLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PET-NEW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    ME::SetProperty( "PetId", "" )
           INVOKE    ME::SetProperty( "PetName", "" )
           INVOKE    ME::SetProperty( "PetRace", "" )
           INVOKE    ME::SetProperty( "PetAge", "0" )
           INVOKE    ME::SetProperty( "PetThumb", "" )
           INVOKE    ME::OpenFormSync( "PET-FORM" )
           INVOKE    ME::GetProperty( "PetSaved" ) RETURNING WS-ANSWER
           IF WS-ANSWER NOT = "Y"
               GOBACK
           END-IF
           SET     WS-NOW                        TO    FUNCTION CURRENT-DATE
           SET     WS-ID                         TO    WS-NOW( 1 : 14 )

           *> PC-PET-STORE: stores the pet the popup returned: photo to S3, item to DynamoDB.
           *> Uses:   WS-ID = the id of the pet (a new one here); the properties PetName, PetRace,
           *>         PetAge and
           *>         PetPhoto the popup left on this form.
           *> Result: The object photos/<id>.jpg and the item are stored; WS-OK = "OK" or the
           *>         reason; the grid
           *>         is reloaded.

           CALL    "PC-PET-STORE"

           GOBACK.

       END PROGRAM PC-PET-NEW.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PET-STORE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    ME::GetProperty( "PetName" ) RETURNING WS-PET-NAME
           INVOKE    ME::GetProperty( "PetRace" ) RETURNING WS-PET-RACE
           INVOKE    ME::GetProperty( "PetAge" ) RETURNING WS-PET-AGE
           INVOKE    ME::GetProperty( "PetPhoto" ) RETURNING WS-PET-PHOTO
           SET     WS-PHOTO-KEY                  TO    SPACES
           SET     WS-OK                         TO    "OK"
           IF WS-PET-PHOTO NOT = SPACES

               *> Without a bucket, say what to do rather than ask S3 for a bucket named "".
               *> PC-BUCKET-ENSURE: the photo bucket into AWS-S3::Bucket - typed, or from the stack.
               *> Result: AWS-S3::Bucket, SPACES when none is known.

               CALL    "PC-BUCKET-ENSURE"
               IF AWS-S3::Bucket = SPACES
                   SET     WS-DDB-MSG                    TO    M-NO-BUCKET
                   SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()
                   GOBACK
               END-IF

               *> The photo is cropped and resized to 400 x 400, then kept in the bucket as
               *> photos/<id>.jpg.

               STRING "data/cache/" WS-ID::Trim() "-400.jpg" INTO WS-LOCAL
               COBOL::"IMAGE-FIT" ( WS-PET-PHOTO::Trim() WS-LOCAL::Trim() 400 400 WS-OK )
               IF WS-OK = "OK"
                   STRING "photos/" WS-ID::Trim() ".jpg" INTO WS-PHOTO-KEY
                   SET     WS-OUT                        TO    AWS-S3::PutObject( WS-PHOTO-KEY::Trim() "" WS-LOCAL::Trim() )
                   IF AWS-S3::LastError NOT = SPACES
                       SET     WS-OK                         TO    AWS-S3::LastError
                   ELSE
                       STRING "data/cache/" WS-ID::Trim() "-40.png" INTO WS-THUMB
                       COBOL::"IMAGE-FIT" ( WS-LOCAL::Trim() WS-THUMB::Trim() 40 40 WS-OK )
                   END-IF
               END-IF
           END-IF
           IF WS-OK NOT = "OK"
               SET     WS-DDB-MSG                    TO    M-PHOTO-NOT-STORED
               SET     WS-DDB-A1                     TO    WS-OK::Trim()
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )
               GOBACK
           END-IF

           *> An edit without a new photo keeps the key the item already has.

           IF WS-PHOTO-KEY = SPACES
               SET     WS-OUT                        TO    AWS-DDB::GetItem( '{"id":"' & WS-ID::Trim() & '"}' )
               IF AWS-DDB::Found = 1
                   SET     WS-PHOTO-KEY                  TO    AWS-DDB::GetField( 1 "photo" )
               END-IF
           END-IF
           SET     WS-JSON                       TO    SPACES
           IF WS-PET-AGE = SPACES
               SET     WS-PET-AGE                    TO    "0"
           END-IF
           STRING '{"id":"' WS-ID::Trim() '","name":"' WS-PET-NAME::Trim() '","race":"' WS-PET-RACE::Trim()
                  '","age":' WS-PET-AGE::Trim() ',"photo":"' WS-PHOTO-KEY::Trim() '"}' INTO WS-JSON
           SET     WS-OUT                        TO    AWS-DDB::PutItem( WS-JSON::Trim() )
           IF AWS-DDB::LastError NOT = SPACES
               SET     WS-DDB-MSG                    TO    M-PET-NOT-STORED
               SET     WS-DDB-A1                     TO    AWS-DDB::LastError::Trim()
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )
               GOBACK
           END-IF

           *> PC-PETS-LOAD: reads every pet of the table into the grid.
           *> Uses:   AWS-DDB (table PowerCloudPets, in the region of the Settings tab).
           *> Result: DG-PETS holds one row per pet, the one just changed included; WS-PET-TABLE is
           *>         filled again;
           *>         the thumbnail timer starts; LBL-DDBSTATE says how many pets, or what went
           *> wrong.

           CALL    "PC-PETS-LOAD"

           GOBACK.

       END PROGRAM PC-PET-STORE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PET-LOAD-ONE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Reads the item WS-ID and leaves its fields on this form for a popup.

           SET     WS-OUT                        TO    AWS-DDB::GetItem( '{"id":"' & WS-ID::Trim() & '"}' )
           IF AWS-DDB::Found NOT = 1
               SET     WS-DDB-MSG                    TO    M-PET-GONE
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()
               SET     WS-OK                         TO    "GONE"
               GOBACK
           END-IF
           SET     WS-OK                         TO    "OK"
           SET     WS-PHOTO-KEY                  TO    AWS-DDB::GetField( 1 "photo" )
           INVOKE    ME::SetProperty( "PetId", WS-ID::Trim() )
           INVOKE    ME::SetProperty( "PetName", AWS-DDB::GetField( 1 "name" ) )
           INVOKE    ME::SetProperty( "PetRace", AWS-DDB::GetField( 1 "race" ) )
           INVOKE    ME::SetProperty( "PetAge", AWS-DDB::GetField( 1 "age" ) )

           GOBACK.

       END PROGRAM PC-PET-LOAD-ONE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PET-VIEW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-PET-LOAD-ONE: reads the item WS-ID and leaves its fields on this form for the
           *> popup.
           *> Uses:   WS-ID = the pet clicked.
           *> Result: WS-OK = "OK" and the properties PetId, PetName, PetRace, PetAge are set, or
           *>         WS-OK = "GONE".

           CALL    "PC-PET-LOAD-ONE"
           IF WS-OK NOT = "OK"
               GOBACK
           END-IF

           *> PC-PHOTO-DOWNLOAD: brings the 400 x 400 photo of the pet WS-ID into the local cache.
           *> Uses:   WS-ID = the pet, WS-PHOTO-KEY = its object key.
           *> Result: WS-LOCAL = the cache path; WS-OK = "OK" when the file is there.

           CALL    "PC-PHOTO-DOWNLOAD"
           IF WS-OK = "OK"
               INVOKE    ME::SetProperty( "ViewPhoto", WS-LOCAL::Trim() )
           ELSE
               INVOKE    ME::SetProperty( "ViewPhoto", "" )
           END-IF
           INVOKE    ME::OpenFormSync( "VIEW-FORM" )

           GOBACK.

       END PROGRAM PC-PET-VIEW.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PET-EDIT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-PET-LOAD-ONE: reads the item WS-ID and leaves its fields on this form for the
           *> popup.
           *> Uses:   WS-ID = the pet clicked.
           *> Result: WS-OK = "OK" and the properties PetId, PetName, PetRace, PetAge are set, or
           *>         WS-OK = "GONE".

           CALL    "PC-PET-LOAD-ONE"
           IF WS-OK NOT = "OK"
               GOBACK
           END-IF
           STRING "data/cache/" WS-ID::Trim() "-40.png" INTO WS-THUMB
           INVOKE    ME::SetProperty( "PetThumb", WS-THUMB::Trim() )
           INVOKE    ME::OpenFormSync( "PET-FORM" )
           INVOKE    ME::GetProperty( "PetSaved" ) RETURNING WS-ANSWER
           IF WS-ANSWER = "Y"

               *> PC-PET-STORE: stores the pet the popup returned: photo to S3, item to DynamoDB.
               *> Uses:   WS-ID = the pet edited; the properties PetName, PetRace, PetAge and
               *>         PetPhoto left by the popup.
               *> Result: The item (and the photo, when a new one was chosen) is stored; the grid is
               *>         reloaded.

               CALL    "PC-PET-STORE"
           END-IF

           GOBACK.

       END PROGRAM PC-PET-EDIT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PET-DELETE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    ME::SetProperty( "ConfirmText", T-CONFIRM-DELETE::Trim() )
           INVOKE    ME::SetProperty( "ConfirmAnswer", "N" )
           INVOKE    ME::OpenFormSync( "CONFIRM-FORM" )
           INVOKE    ME::GetProperty( "ConfirmAnswer" ) RETURNING WS-ANSWER
           IF WS-ANSWER NOT = "Y"
               GOBACK
           END-IF
           STRING "photos/" WS-ID::Trim() ".jpg" INTO WS-PHOTO-KEY

           *> PC-BUCKET-ENSURE: the photo bucket into AWS-S3::Bucket - typed, or from the stack.
           *> Result: AWS-S3::Bucket, SPACES when none is known.

           CALL    "PC-BUCKET-ENSURE"
           SET     WS-OUT                        TO    AWS-S3::DeleteObject( WS-PHOTO-KEY::Trim() )
           SET     WS-OUT                        TO    AWS-DDB::DeleteItem( '{"id":"' & WS-ID::Trim() & '"}' )
           IF AWS-DDB::LastError NOT = SPACES
               SET     WS-DDB-MSG                    TO    M-PET-NOT-DELETED
               SET     WS-DDB-A1                     TO    AWS-DDB::LastError::Trim()
               SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )
               GOBACK
           END-IF

           *> PC-PETS-LOAD: reads every pet of the table into the grid.
           *> Uses:   AWS-DDB (table PowerCloudPets, in the region of the Settings tab).
           *> Result: DG-PETS holds one row per pet, the one just changed included; WS-PET-TABLE is
           *>         filled again;
           *>         the thumbnail timer starts; LBL-DDBSTATE says how many pets, or what went
           *> wrong.

           CALL    "PC-PETS-LOAD"

           GOBACK.

       END PROGRAM PC-PET-DELETE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-S3-BUCKETS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-OUT                        TO    AWS-S3-LIST::ListBuckets()
           IF AWS-S3-LIST::LastError NOT = SPACES
               SET     WS-S3-MSG                     TO    M-BUCKETS-NOT-LISTED
               SET     WS-S3-A1                      TO    AWS-S3-LIST::LastError::Trim()
               SET     LBL-S3STATE::Caption          TO    PC-TEXT-NOW(WS-S3-MSG)::Trim()::Replace( "&1", WS-S3-A1::Trim() )
               GOBACK
           END-IF
           CMB-BUCKET::Clear()
           SET     WS-N                          TO    AWS-S3-LIST::RowCount
           PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > WS-N
               CMB-BUCKET::AddItem( AWS-S3-LIST::GetField( WS-ROW "Name" ) )
           END-PERFORM
           IF WS-N > 0
               CMB-BUCKET::SetSelectedIndex( 1 )

               *> PC-S3-LIST: lists the objects of the bucket picked.
               *> Uses:   CMB-BUCKET::GetSelected() = the first bucket.
               *> Result: DG-S3 holds one row per object; LBL-S3STATE says how many.

               CALL    "PC-S3-LIST"
           ELSE
               SET     WS-S3-MSG                     TO    M-NO-BUCKETS
               SET     LBL-S3STATE::Caption          TO    PC-TEXT-NOW(WS-S3-MSG)::Trim()
           END-IF

           GOBACK.

       END PROGRAM PC-S3-BUCKETS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-S3-LIST IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     AWS-S3-LIST::Bucket           TO    CMB-BUCKET::GetSelected()
           SET     WS-OUT                        TO    AWS-S3-LIST::List( "" )
           IF AWS-S3-LIST::LastError NOT = SPACES
               SET     WS-S3-MSG                     TO    M-BUCKET-NOT-LISTED
               SET     WS-S3-A1                      TO    AWS-S3-LIST::LastError::Trim()
               SET     LBL-S3STATE::Caption          TO    PC-TEXT-NOW(WS-S3-MSG)::Trim()::Replace( "&1", WS-S3-A1::Trim() )
               GOBACK
           END-IF
           DG-S3::ClearRows()
           SET     WS-N                          TO    AWS-S3-LIST::RowCount
           PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > WS-N
               SET     WS-ROWTXT                     TO    SPACES
               STRING AWS-S3-LIST::GetField( WS-ROW "Key" ) X"09" AWS-S3-LIST::GetField( WS-ROW "Size" ) X"09"
                      AWS-S3-LIST::GetField( WS-ROW "LastModified" ) INTO WS-ROWTXT
               DG-S3::AddRow( WS-ROWTXT::Trim() )
           END-PERFORM
           SET     WS-NUM                        TO    WS-N
           SET     WS-S3-MSG                     TO    M-OBJECTS-COUNT
           SET     WS-S3-A1                      TO    WS-NUM::Trim()
           SET     WS-S3-A2                      TO    CMB-BUCKET::GetSelected()
           SET     LBL-S3STATE::Caption          TO    PC-TEXT-NOW(WS-S3-MSG)::Trim()::Replace( "&1", WS-S3-A1::Trim() )::Replace( "&2", WS-S3-A2::Trim() )

           GOBACK.

       END PROGRAM PC-S3-LIST.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-EC2-LOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-OUT                        TO    AWS-EC2::Describe( "" )
           IF AWS-EC2::LastError NOT = SPACES
               SET     WS-EC2-MSG                    TO    M-SERVERS-NOT-LISTED
               SET     WS-EC2-A1                     TO    AWS-EC2::LastError::Trim()
               SET     LBL-EC2STATE::Caption         TO    PC-TEXT-NOW(WS-EC2-MSG)::Trim()::Replace( "&1", WS-EC2-A1::Trim() )
               GOBACK
           END-IF
           DG-EC2::ClearRows()
           SET     WS-N                          TO    AWS-EC2::RowCount
           PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > WS-N
               SET     WS-EC2-TYPE                   TO    AWS-EC2::GetField( WS-ROW "Type" )
               SET     WS-EC2-STATE                  TO    AWS-EC2::GetField( WS-ROW "State" )
               SET     WS-EC2-CPU                    TO    0
               SET     WS-EC2-MEM                    TO    "?"
               PERFORM VARYING WS-T FROM 1 BY 1 UNTIL WS-T > 12
                   IF WS-TYPE-NAME(WS-T) = WS-EC2-TYPE
                       SET     WS-EC2-CPU                    TO    WS-TYPE-CPU(WS-T)
                       SET     WS-EC2-MEM                    TO    WS-TYPE-MEM(WS-T)
                   END-IF
               END-PERFORM

               *> A server that is running or starting reads as "on".

               IF WS-EC2-STATE = "running" OR WS-EC2-STATE = "pending"
                   SET     WS-EC2-POWER                  TO    "1"
               ELSE
                   SET     WS-EC2-POWER                  TO    "0"
               END-IF
               SET     WS-CPU-TXT                    TO    WS-EC2-CPU
               SET     WS-ROWTXT                     TO    SPACES
               STRING AWS-EC2::GetField( WS-ROW "InstanceId" ) X"09" AWS-EC2::GetField( WS-ROW "Name" ) X"09"
                      WS-EC2-TYPE::Trim() X"09" WS-CPU-TXT::Trim() X"09" WS-EC2-MEM::Trim() X"09" WS-EC2-STATE::Trim() X"09" WS-EC2-POWER
                      INTO WS-ROWTXT
               DG-EC2::AddRow( WS-ROWTXT::Trim() )
           END-PERFORM
           SET     WS-NUM                        TO    WS-N
           SET     WS-EC2-MSG                    TO    M-SERVERS-COUNT
           SET     WS-EC2-A1                     TO    WS-NUM::Trim()
           SET     WS-EC2-A2                     TO    AWS-EC2::Connection
           SET     LBL-EC2STATE::Caption         TO    PC-TEXT-NOW(WS-EC2-MSG)::Trim()::Replace( "&1", WS-EC2-A1::Trim() )::Replace( "&2", WS-EC2-A2::Trim() )

           GOBACK.

       END PROGRAM PC-EC2-LOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-REK-RUN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           IF WS-REK-FILE = SPACES
               SET     WS-REK-LIST                   TO    "N"
               SET     WS-REK-MSG                    TO    M-REK-DROP-FIRST
               SET     LBL-REKDETAILS::Caption       TO    PC-TEXT-NOW(WS-REK-MSG)::Trim()
               GOBACK
           END-IF
           SET     WS-REK-LIST                   TO    "N"
           SET     WS-REK-MSG                    TO    M-REK-ASKING
           SET     LBL-REKDETAILS::Caption       TO    PC-TEXT-NOW(WS-REK-MSG)::Trim()
           SET     WS-OUT                        TO    AWS-REK::RecognizeCelebrities( WS-REK-FILE::Trim() )
           IF AWS-REK::LastError NOT = SPACES
               SET     WS-REK-MSG                    TO    M-REK-SAID
               SET     WS-REK-A1                     TO    AWS-REK::LastError::Trim()
               SET     LBL-REKDETAILS::Caption       TO    PC-TEXT-NOW(WS-REK-MSG)::Trim()::Replace( "&1", WS-REK-A1::Trim() )
               GOBACK
           END-IF
           SET     WS-N                          TO    AWS-REK::RowCount
           IF WS-N = 0
               SET     WS-REK-MSG                    TO    M-REK-NONE
               SET     LBL-REKDETAILS::Caption       TO    PC-TEXT-NOW(WS-REK-MSG)::Trim()
               GOBACK
           END-IF

           *> Where the picture really is inside PIC-REK (480 x 420 at 48,262): Zoom keeps its
           *> proportions,
           *> so it fills one side and leaves bands on the other.

           COBOL::"IMAGE-INFO" ( WS-REK-FILE::Trim() WS-PIC-W WS-PIC-H WS-FMT WS-OK )
           COMPUTE WS-SCALE = 480 / WS-PIC-W
           COMPUTE WS-SCALE-H = 420 / WS-PIC-H
           IF WS-SCALE-H < WS-SCALE
               SET     WS-SCALE                      TO    WS-SCALE-H
           END-IF
           COMPUTE WS-DISP-W = WS-PIC-W * WS-SCALE
           COMPUTE WS-DISP-H = WS-PIC-H * WS-SCALE
           COMPUTE WS-OFF-X = 48 + (480 - WS-DISP-W) / 2
           COMPUTE WS-OFF-Y = 262 + (420 - WS-DISP-H) / 2

           PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > WS-N OR WS-ROW > 6
               COMPUTE WS-BX = WS-OFF-X + FUNCTION NUMVAL( AWS-REK::GetField( WS-ROW "Left" ) ) * WS-DISP-W
               COMPUTE WS-BY = WS-OFF-Y + FUNCTION NUMVAL( AWS-REK::GetField( WS-ROW "Top" ) ) * WS-DISP-H
               COMPUTE WS-BW = FUNCTION NUMVAL( AWS-REK::GetField( WS-ROW "Width" ) ) * WS-DISP-W
               COMPUTE WS-BH = FUNCTION NUMVAL( AWS-REK::GetField( WS-ROW "Height" ) ) * WS-DISP-H
               EVALUATE WS-ROW
                   WHEN 1
                       SHP-BOX1::MoveTo( WS-BX WS-BY )
                       SHP-BOX1::Resize( WS-BW WS-BH )
                       SET     SHP-BOX1::Visible           TO    TRUE
                   WHEN 2
                       SHP-BOX2::MoveTo( WS-BX WS-BY )
                       SHP-BOX2::Resize( WS-BW WS-BH )
                       SET     SHP-BOX2::Visible           TO    TRUE
                   WHEN 3
                       SHP-BOX3::MoveTo( WS-BX WS-BY )
                       SHP-BOX3::Resize( WS-BW WS-BH )
                       SET     SHP-BOX3::Visible           TO    TRUE
                   WHEN 4
                       SHP-BOX4::MoveTo( WS-BX WS-BY )
                       SHP-BOX4::Resize( WS-BW WS-BH )
                       SET     SHP-BOX4::Visible           TO    TRUE
                   WHEN 5
                       SHP-BOX5::MoveTo( WS-BX WS-BY )
                       SHP-BOX5::Resize( WS-BW WS-BH )
                       SET     SHP-BOX5::Visible           TO    TRUE
                   WHEN 6
                       SHP-BOX6::MoveTo( WS-BX WS-BY )
                       SHP-BOX6::Resize( WS-BW WS-BH )
                       SET     SHP-BOX6::Visible           TO    TRUE
               END-EVALUATE
           END-PERFORM
           SET     WS-REK-LIST                   TO    "Y"

           *> PC-REK-DETAILS: writes the celebrities found, with each match, into LBL-REKDETAILS.
           *> Uses:   AWS-REK (the answer just received) and T-REK-MATCH.
           *> Result: LBL-REKDETAILS::Caption = one block per celebrity (at most six); WS-DETAILS,
           *>         WS-DPOS, WS-MATCH, WS-MATCH-VAL, WS-N and WS-ROW are overwritten.

           CALL    "PC-REK-DETAILS"

           GOBACK.

       END PROGRAM PC-REK-RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TEXTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The language picked in CMB-LANGUAGE (WS-LANG-IX): every text of the form in it - the
           *> designed
           *> captions and hints, the tab titles, the headings of the grids, the voices, and the
           *> status
           *> lines, which are written again from the message each one last showed.

           IF WS-LANG-IX < 1 OR WS-LANG-IX > 6
               SET     WS-LANG-IX                    TO    1
           END-IF
           PERFORM VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > WS-TEXT-ROWS
               SET     PC-TEXT-NOW(WS-TX-I)          TO    PC-TEXT(WS-TX-I, WS-LANG-IX)
           END-PERFORM
           SET     T-SET-HINT                    TO    PC-LONG(1, WS-LANG-IX)
           SET     T-LAM-TEXT                    TO    PC-LONG(2, WS-LANG-IX)

           *> The popups ask for the language when they open: it is a property of this form.

           INVOKE    ME::SetProperty( "UiLanguage", WS-LANG-IX )

           *> Title bar and tab titles.

           SET     LBL-TAGLINE::Caption          TO    T-TAGLINE::Trim()
           SET     CMB-LANGUAGE::Tooltip         TO    T-LANG-TIP::Trim()
           SET     TAB-MAIN::Tabs                TO    T-TAB-SETTINGS::Trim() & WS-NL & "DynamoDB" & WS-NL & "S3" & WS-NL & "Lambda" & WS-NL & "Polly" & WS-NL & "EC2" & WS-NL & "Rekognition"

           *> Settings tab.

           SET     LBL-SETHEAD::Caption          TO    T-SET-HEAD::Trim()
           SET     LBL-SETHINT::Caption          TO    T-SET-HINT::Trim()
           SET     LBL-REGION::Caption           TO    T-REGION::Trim()
           SET     LBL-KEYID::Caption            TO    T-KEY-ID::Trim()
           SET     LBL-TOKEN::Caption            TO    T-TOKEN::Trim()
           SET     TXT-TOKEN::HintText           TO    T-TOKEN-HINT::Trim()
           SET     LBL-SECRET::Caption           TO    T-SECRET::Trim()
           SET     BTN-CONNECT::Caption          TO    T-CONNECT::Trim()
           SET     LBL-STACK::Caption            TO    T-STACK-NAME::Trim()
           SET     LBL-BUCKET::Caption           TO    T-BUCKET::Trim()
           SET     TXT-BUCKET::HintText          TO    T-BUCKET-HINT::Trim()
           SET     LBL-FUNCTION::Caption         TO    T-FUNCTION::Trim()
           SET     BTN-DEPLOY::Caption           TO    T-DEPLOY::Trim()
           SET     BTN-SAVE::Caption             TO    T-SAVE-TEMPLATE::Trim()
           SET     LBL-LINK::Caption             TO    T-LINK-TEMPLATE::Trim()

           *> DynamoDB tab.

           SET     LBL-DDBHEAD::Caption          TO    T-DDB-HEAD::Trim()
           SET     BTN-CREATETABLE::Caption      TO    T-CREATE-TABLE::Trim()
           SET     BTN-NEWPET::Caption           TO    T-NEW-PET::Trim()
           SET     BTN-RELOAD::Caption           TO    T-RELOAD::Trim()
           SET     BTN-CLEARFILTERS::Caption     TO    T-CLEAR-FILTERS::Trim()
           DG-PETS::SetColumnTitle( "Col-Id", T-COL-ID::Trim() )
           DG-PETS::SetColumnTitle( "Col-Photo", T-COL-PHOTO::Trim() )
           DG-PETS::SetColumnTitle( "Col-Name", T-COL-NAME::Trim() )
           DG-PETS::SetColumnTitle( "Col-Race", T-COL-RACE::Trim() )
           DG-PETS::SetColumnTitle( "Col-Age", T-COL-AGE::Trim() )
           DG-PETS::SetColumnTitle( "Col-View", T-COL-VIEW::Trim() )
           DG-PETS::SetColumnTitle( "Col-Edit", T-COL-EDIT::Trim() )
           DG-PETS::SetColumnTitle( "Col-Delete", T-COL-DELETE::Trim() )

           *> S3 tab.

           SET     LBL-S3HEAD::Caption           TO    T-S3-HEAD::Trim()
           SET     BTN-S3RELOAD::Caption         TO    T-RELOAD::Trim()
           DG-S3::SetColumnTitle( "Col-Key", T-COL-KEY::Trim() )
           DG-S3::SetColumnTitle( "Col-Size", T-COL-SIZE::Trim() )
           DG-S3::SetColumnTitle( "Col-Modified", T-COL-MODIFIED::Trim() )

           *> Lambda tab.

           SET     LBL-LAMHEAD::Caption          TO    T-LAM-HEAD::Trim()
           SET     LBL-LAMTEXT::Caption          TO    T-LAM-TEXT::Trim()
           SET     LBL-LAMSRC::Caption           TO    T-LAM-SRC::Trim()
           SET     LBL-LAMKEY::Caption           TO    T-LAM-KEY::Trim()
           SET     BTN-LAMTRY::Caption           TO    T-LAM-TRY::Trim()

           *> Polly tab. The list of voices is built again in the new language and keeps the voice
           *> picked.

           SET     LBL-POLLYHEAD::Caption        TO    T-POLLY-HEAD::Trim()
           SET     LBL-LANG::Caption             TO    T-POLLY-LANG::Trim()
           SET     BTN-SPEAK::Caption            TO    T-SPEAK::Trim()
           SET     WS-VOICE-IX                   TO    CMB-LANG::GetSelectedIndex()
           IF WS-VOICE-IX < 1
               SET     WS-VOICE-IX                   TO    1
           END-IF
           CMB-LANG::Clear()
           CMB-LANG::AddItem( T-VOICE-1::Trim() )
           CMB-LANG::AddItem( T-VOICE-2::Trim() )
           CMB-LANG::AddItem( T-VOICE-3::Trim() )
           CMB-LANG::AddItem( T-VOICE-4::Trim() )
           CMB-LANG::AddItem( T-VOICE-5::Trim() )
           CMB-LANG::AddItem( T-VOICE-6::Trim() )
           CMB-LANG::SetSelectedIndex( WS-VOICE-IX )

           *> EC2 tab.

           SET     LBL-EC2HEAD::Caption          TO    T-EC2-HEAD::Trim()
           SET     BTN-EC2RELOAD::Caption        TO    T-RELOAD::Trim()
           DG-EC2::SetColumnTitle( "Col-Instance", T-COL-INSTANCE::Trim() )
           DG-EC2::SetColumnTitle( "Col-Name", T-COL-NAME::Trim() )
           DG-EC2::SetColumnTitle( "Col-Class", T-COL-CLASS::Trim() )
           DG-EC2::SetColumnTitle( "Col-Vcpu", T-COL-VCPU::Trim() )
           DG-EC2::SetColumnTitle( "Col-Memory", T-COL-MEMORY::Trim() )
           DG-EC2::SetColumnTitle( "Col-State", T-COL-STATE::Trim() )
           DG-EC2::SetColumnTitle( "Col-Power", T-COL-POWER::Trim() )

           *> Rekognition tab.

           SET     LBL-REKHEAD::Caption          TO    T-REK-HEAD::Trim()
           SET     DRP-REK::Hint                 TO    T-DROP-HINT::Trim()
           SET     BTN-RECOGNIZE::Caption        TO    T-RECOGNIZE::Trim()

           *> The status lines, from the message each one last showed and the values that went into
           *> it.

           SET     LBL-CONN::Caption             TO    PC-TEXT-NOW(WS-CONN-MSG)::Trim()::Replace( "&1", WS-CONN-A1::Trim() )::Replace( "&2", WS-CONN-A2::Trim() )
           SET     LBL-STACKSTATE::Caption       TO    PC-TEXT-NOW(WS-STK-MSG)::Trim()::Replace( "&1", WS-STK-A1::Trim() )::Replace( "&2", WS-STK-A2::Trim() )
           SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )::Replace( "&2", WS-DDB-A2::Trim() )
           SET     LBL-S3STATE::Caption          TO    PC-TEXT-NOW(WS-S3-MSG)::Trim()::Replace( "&1", WS-S3-A1::Trim() )::Replace( "&2", WS-S3-A2::Trim() )
           SET     LBL-LAMRESULT::Caption        TO    PC-TEXT-NOW(WS-LAM-MSG)::Trim()::Replace( "&1", WS-LAM-A1::Trim() )::Replace( "&2", WS-LAM-A2::Trim() )
           SET     LBL-POLLYSTATE::Caption       TO    PC-TEXT-NOW(WS-POL-MSG)::Trim()::Replace( "&1", WS-POL-A1::Trim() )::Replace( "&2", WS-POL-A2::Trim() )
           SET     LBL-EC2STATE::Caption         TO    PC-TEXT-NOW(WS-EC2-MSG)::Trim()::Replace( "&1", WS-EC2-A1::Trim() )::Replace( "&2", WS-EC2-A2::Trim() )
           IF WS-REK-LIST = "Y"

               *> PC-REK-DETAILS: writes the celebrities the last call found, with each match, into
               *> LBL-REKDETAILS.
               *> Uses:   AWS-REK (the answer to the last RecognizeCelebrities call) and
               *>         T-REK-MATCH, which is
               *>         now in the new language.
               *> Result: LBL-REKDETAILS::Caption = the list, in the new language; WS-DETAILS,
               *>         WS-DPOS,
               *>         WS-MATCH, WS-MATCH-VAL, WS-N and WS-ROW are overwritten.

               CALL    "PC-REK-DETAILS"
           ELSE
               SET     LBL-REKDETAILS::Caption       TO    PC-TEXT-NOW(WS-REK-MSG)::Trim()::Replace( "&1", WS-REK-A1::Trim() )::Replace( "&2", WS-REK-A2::Trim() )
           END-IF

           GOBACK.

       END PROGRAM PC-TEXTS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-REK-DETAILS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The celebrities of the last call, one block each: name, match and links.

           SET     WS-DETAILS                    TO    SPACES
           SET     WS-DPOS                       TO    1
           SET     WS-N                          TO    AWS-REK::RowCount
           PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > WS-N OR WS-ROW > 6
               SET     WS-MATCH-VAL                  TO    AWS-REK::GetField( WS-ROW "MatchConfidence" )
               SET     WS-MATCH                      TO    T-REK-MATCH::Trim()::Replace( "&1", WS-MATCH-VAL::Trim() )
               STRING AWS-REK::GetField( WS-ROW "Name" ) WS-NL WS-MATCH::Trim() WS-NL
                      AWS-REK::GetField( WS-ROW "Urls" ) WS-NL WS-NL
                      DELIMITED BY SIZE INTO WS-DETAILS WITH POINTER WS-DPOS
           END-PERFORM
           SET     LBL-REKDETAILS::Caption       TO    WS-DETAILS::Trim()

           GOBACK.

       END PROGRAM PC-REK-DETAILS.

       END PROGRAM MAIN-FORM.
