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
       01 WS-HTTP         PIC 999     GLOBAL.
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
          05 WS-Lbl-LamText-TEXT       PIC X(478) VALUE 'PowerCloud never signs a download link itself. When the grid or the pet''s popup needs a photo, the application asks a small function, powercloud-presign, for a link to that one object. The function runs with a role that can only read the photo bucket, and the link stops working after 15 minutes. The application then saves the bytes behind the link into its local cache with HTTP-DOWNLOAD. AwsLambda.Invoke needs AllowWrite on, because a function can do whatever its code says.'.
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

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

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
           *> memory only, and the boxes are emptied at once.

           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 6
               COBOL::"AWS-SET-CREDENTIALS" ( WS-REGION(WS-I) TXT-KEYID::Text TXT-SECRET::Text TXT-TOKEN::Text WS-STATUS )
           END-PERFORM
           SET     TXT-KEYID::Text               TO    SPACES
           SET     TXT-SECRET::Text              TO    SPACES
           SET     TXT-TOKEN::Text               TO    SPACES

           IF WS-STATUS NOT = "OK" AND WS-STATUS NOT = SPACES
               SET     LBL-CONN::Caption             TO    "The keys were not accepted: " & WS-STATUS::Trim()
               GOBACK
           END-IF

           *> One read-only call proves who the keys belong to.

           SET     LBL-CONN::Caption             TO    "Checking the keys…"
           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "sts" "GetCallerIdentity" "{}" )
           IF AWS-MCP::LastError NOT = SPACES
               SET     LBL-CONN::Caption             TO    "Not connected: " & AWS-MCP::LastError::Trim()
           ELSE
               SET     LBL-CONN::Caption             TO    "Connected as " & AWS-MCP::GetField( 1 "Arn" )::Trim()
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
               SET     LBL-STACKSTATE::Caption       TO    "The template file could not be read: " & WS-STATUS::Trim()
               GOBACK
           END-IF

           STRING '{"StackName":"' TXT-STACK::Text::Trim() '","Capabilities":["CAPABILITY_IAM"],'
                  '"Parameters":[{"ParameterKey":"FunctionName","ParameterValue":"' TXT-FUNCTION::Text::Trim() '"}],'
                  '"TemplateBody":"' WS-TEMPLATE::Trim() '"}'
                  DELIMITED BY SIZE INTO WS-PARAMS

           SET     LBL-STACKSTATE::Caption       TO    "Creating the stack…"
           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "cloudformation" "CreateStack" WS-PARAMS::Trim() )
           IF AWS-MCP::LastError NOT = SPACES
               SET     LBL-STACKSTATE::Caption       TO    "CloudFormation refused it: " & AWS-MCP::LastError::Trim()
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
               SET     LBL-DDBSTATE::Caption         TO    "The table PowerCloudPets already exists."
               GOBACK
           END-IF
           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "dynamodb" "CreateTable" '{"TableName":"PowerCloudPets","AttributeDefinitions":[{"AttributeName":"id","AttributeType":"S"}],"KeySchema":[{"AttributeName":"id","KeyType":"HASH"}],"BillingMode":"PAY_PER_REQUEST"}' )
           IF AWS-MCP::LastError = SPACES
               SET     LBL-DDBSTATE::Caption         TO    "The table PowerCloudPets was created."
           ELSE
               SET     LBL-DDBSTATE::Caption         TO    "The table could not be created: " & AWS-MCP::LastError::Trim()
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
               SET     LBL-LAMRESULT::Caption        TO    "The function could not be called: " & AWS-LAMBDA::LastError::Trim()
               GOBACK
           END-IF
           SET     WS-URL                        TO    WS-OUT::Replace( '": "', '":"' )::Split( '"url":"' )(2)::Split( '"' )(1)
           IF WS-URL = SPACES
               SET     LBL-LAMRESULT::Caption        TO    "The function answered: " & WS-OUT::Trim()
           ELSE

               *> Only the part before the question mark is shown: the signature after it is a
               *> secret for 15 minutes.

               SET     LBL-LAMRESULT::Caption        TO    "Link: " & WS-URL::Split( "?" )(1) & "?... (signature hidden)"
           END-IF

           GOBACK.

       END PROGRAM BTN-LAMTRY--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SPEAK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-VOICE-IX                   TO    CMB-LANG::GetSelectedIndex()
           SET     LBL-POLLYSTATE::Caption       TO    "Asking Polly…"
           SET     WS-OUT                        TO    AWS-POLLY::Synthesize( TXT-POLLYTEXT::Text WS-VOICE(WS-VOICE-IX)::Trim() "mp3" "data/cache/speech.mp3" )
           IF AWS-POLLY::LastError NOT = SPACES
               SET     LBL-POLLYSTATE::Caption       TO    "Polly could not speak: " & AWS-POLLY::LastError::Trim()
               GOBACK
           END-IF
           SET     LBL-POLLYSTATE::Caption       TO    "Ready: " & AWS-POLLY::Characters & " characters."
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
               SET     LBL-EC2STATE::Caption         TO    "AWS refused: " & AWS-EC2::LastError::Trim()

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

           INVOKE    ME::SetProperty( "WorkingText", "Waiting for " & WS-EC2-ID::Trim() & " to be " & WS-TARGET::Trim() & "…" )
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
           SET     LBL-REKDETAILS::Caption       TO    "Press Recognize."
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

           COBOL::"SAVE-FILE-DIALOG" ( "Save the CloudFormation template" "YAML|*.yaml" "powercloud.yaml" "" WS-SAVE-PATH )
           IF WS-SAVE-PATH = SPACES
               GOBACK
           END-IF
           OPEN INPUT TEMPLATE-FILE
           OPEN OUTPUT SAVE-FILE
           IF WS-FS NOT = "00" OR WS-FS-OUT NOT = "00"
               SET     LBL-STACKSTATE::Caption       TO    "The template could not be saved."
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
           SET     LBL-STACKSTATE::Caption       TO    "Template saved to " & WS-SAVE-PATH::Trim()

           GOBACK.

       END PROGRAM PC-SAVE-TEMPLATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-STACK-REFRESH IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> Called by TMR-STACK every three seconds while a stack is being created.

           SET     WS-OUT                        TO    AWS-MCP::AwsCall( "cloudformation" "DescribeStacks" '{"StackName":"powercloud"}' )
           IF AWS-MCP::LastError NOT = SPACES
               SET     LBL-STACKSTATE::Caption       TO    "Waiting for CloudFormation: " & AWS-MCP::LastError::Trim()
               GOBACK
           END-IF
           SET     WS-STACK-STATE                TO    AWS-MCP::GetField( 1 "StackStatus" )
           ADD 7 TO PGB-STACK::Value
           SET     LBL-STACKSTATE::Caption       TO    "Stack " & TXT-STACK::Text::Trim() & ": " & WS-STACK-STATE::Trim()
           IF WS-STACK-STATE = "CREATE_COMPLETE"
               SET     TMR-STACK::Enabled            TO    FALSE
               SET     PGB-STACK::Value              TO    100
               SET     WS-OUTPUTS                    TO    AWS-MCP::GetField( 1 "Outputs" )::Replace( '": "', '":"' )
               SET     TXT-BUCKET::Text              TO    WS-OUTPUTS::Split( '"OutputKey":"BucketName","OutputValue":"' )(2)::Split( '"' )(1)
               SET     TXT-FUNCTION::Text            TO    WS-OUTPUTS::Split( '"OutputKey":"FunctionName","OutputValue":"' )(2)::Split( '"' )(1)
               SET     AWS-S3::Bucket                TO    TXT-BUCKET::Text
               SET     AWS-LAMBDA::FunctionName      TO    TXT-FUNCTION::Text
           END-IF
           IF WS-STACK-STATE(1:6) = "ROLLBA" OR WS-STACK-STATE(1:6) = "CREATE" AND WS-STACK-STATE(8:6) = "FAILED"
               SET     TMR-STACK::Enabled            TO    FALSE
           END-IF

           GOBACK.

       END PROGRAM PC-STACK-REFRESH.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PETS-LOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-OUT                        TO    AWS-DDB::Scan( 200 )
           IF AWS-DDB::LastError NOT = SPACES
               SET     LBL-DDBSTATE::Caption         TO    "The table could not be read: " & AWS-DDB::LastError::Trim()
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
           SET     LBL-DDBSTATE::Caption         TO    WS-NUM::Trim() & " pet(s) in the table."

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

           *> The temporary link comes from the Lambda function; the bytes come from the link.

           SET     WS-JSON                       TO    SPACES
           STRING '{"key":"' WS-PHOTO-KEY::Trim() '"}' INTO WS-JSON
           SET     WS-OUT                        TO    AWS-LAMBDA::Invoke( "" WS-JSON::Trim() )
           IF AWS-LAMBDA::LastError NOT = SPACES
               SET     WS-OK                         TO    "LAMBDA"
               GOBACK
           END-IF

           *> The function answers one JSON object, which is not a list of rows: the link is cut out
           *> of the text.

           SET     WS-URL                        TO    WS-OUT::Replace( '": "', '":"' )::Split( '"url":"' )(2)::Split( '"' )(1)
           IF WS-URL = SPACES
               SET     WS-OK                         TO    "NOURL"
               GOBACK
           END-IF
           COBOL::"HTTP-DOWNLOAD" ( WS-URL::Trim() WS-LOCAL::Trim() WS-HTTP WS-OK )

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
               SET     LBL-DDBSTATE::Caption         TO    "The photo was not stored: " & WS-OK::Trim()
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
               SET     LBL-DDBSTATE::Caption         TO    "The pet was not stored: " & AWS-DDB::LastError::Trim()
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
               SET     LBL-DDBSTATE::Caption         TO    "That pet is no longer in the table."
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

           INVOKE    ME::SetProperty( "ConfirmText", "Delete this pet and its photo?" )
           INVOKE    ME::SetProperty( "ConfirmAnswer", "N" )
           INVOKE    ME::OpenFormSync( "CONFIRM-FORM" )
           INVOKE    ME::GetProperty( "ConfirmAnswer" ) RETURNING WS-ANSWER
           IF WS-ANSWER NOT = "Y"
               GOBACK
           END-IF
           STRING "photos/" WS-ID::Trim() ".jpg" INTO WS-PHOTO-KEY
           SET     WS-OUT                        TO    AWS-S3::DeleteObject( WS-PHOTO-KEY::Trim() )
           SET     WS-OUT                        TO    AWS-DDB::DeleteItem( '{"id":"' & WS-ID::Trim() & '"}' )
           IF AWS-DDB::LastError NOT = SPACES
               SET     LBL-DDBSTATE::Caption         TO    "The pet was not deleted: " & AWS-DDB::LastError::Trim()
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
               SET     LBL-S3STATE::Caption          TO    "The buckets could not be listed: " & AWS-S3-LIST::LastError::Trim()
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
               SET     LBL-S3STATE::Caption          TO    "This account has no buckets in this region."
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
               SET     LBL-S3STATE::Caption          TO    "The bucket could not be listed: " & AWS-S3-LIST::LastError::Trim()
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
           SET     LBL-S3STATE::Caption          TO    WS-NUM::Trim() & " object(s) in " & CMB-BUCKET::GetSelected()

           GOBACK.

       END PROGRAM PC-S3-LIST.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-EC2-LOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-OUT                        TO    AWS-EC2::Describe( "" )
           IF AWS-EC2::LastError NOT = SPACES
               SET     LBL-EC2STATE::Caption         TO    "The servers could not be listed: " & AWS-EC2::LastError::Trim()
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
           SET     LBL-EC2STATE::Caption         TO    WS-NUM::Trim() & " server(s) in " & AWS-EC2::Connection

           GOBACK.

       END PROGRAM PC-EC2-LOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-REK-RUN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           IF WS-REK-FILE = SPACES
               SET     LBL-REKDETAILS::Caption       TO    "Drop a photo first."
               GOBACK
           END-IF
           SET     LBL-REKDETAILS::Caption       TO    "Asking Rekognition…"
           SET     WS-OUT                        TO    AWS-REK::RecognizeCelebrities( WS-REK-FILE::Trim() )
           IF AWS-REK::LastError NOT = SPACES
               SET     LBL-REKDETAILS::Caption       TO    "Rekognition said: " & AWS-REK::LastError::Trim()
               GOBACK
           END-IF
           SET     WS-N                          TO    AWS-REK::RowCount
           IF WS-N = 0
               SET     LBL-REKDETAILS::Caption       TO    "No celebrity was recognised in this picture."
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

           SET     WS-DETAILS                    TO    SPACES
           SET     WS-DPOS                       TO    1
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
               STRING AWS-REK::GetField( WS-ROW "Name" ) WS-NL "Match: " AWS-REK::GetField( WS-ROW "MatchConfidence" ) " %" WS-NL
                      AWS-REK::GetField( WS-ROW "Urls" ) WS-NL WS-NL
                      DELIMITED BY SIZE INTO WS-DETAILS WITH POINTER WS-DPOS
           END-PERFORM
           SET     LBL-REKDETAILS::Caption       TO    WS-DETAILS::Trim()

           GOBACK.

       END PROGRAM PC-REK-RUN.

       END PROGRAM MAIN-FORM.
