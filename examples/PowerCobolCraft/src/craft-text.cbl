       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRAFT-TEXT.

      *> The texts the player reads on the title screen and in the world, in six languages:
      *> 1 English, 2 Portuguese (Brazil), 3 Spanish, 4 French, 5 Japanese, 6 Chinese (simplified).
      *> One row per text, one column per language. The messages of the server console and of its
      *> commands are not here: they are CobolCraft's own words, kept in English as the original prints them.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-TEXT-DATA.
      *> Text 1: Play a new world
           05 FILLER                             PIC    X(240)    VALUE
               "Play a new world".
           05 FILLER                             PIC    X(240)    VALUE
               "Jogar um mundo novo".
           05 FILLER                             PIC    X(240)    VALUE
               "Jugar un mundo nuevo".
           05 FILLER                             PIC    X(240)    VALUE
               "Jouer dans un nouveau monde".
           05 FILLER                             PIC    X(240)    VALUE
               "新しいワールドで遊ぶ".
           05 FILLER                             PIC    X(240)    VALUE
               "开始新世界".
      *> Text 2: Continue the saved world
           05 FILLER                             PIC    X(240)    VALUE
               "Continue the saved world".
           05 FILLER                             PIC    X(240)    VALUE
               "Continuar o mundo salvo".
           05 FILLER                             PIC    X(240)    VALUE
               "Continuar el mundo guardado".
           05 FILLER                             PIC    X(240)    VALUE
               "Reprendre le monde enregistré".
           05 FILLER                             PIC    X(240)    VALUE
               "保存したワールドを続ける".
           05 FILLER                             PIC    X(240)    VALUE
               "继续已保存的世界".
      *> Text 3: Quit
           05 FILLER                             PIC    X(240)    VALUE
               "Quit".
           05 FILLER                             PIC    X(240)    VALUE
               "Sair".
           05 FILLER                             PIC    X(240)    VALUE
               "Salir".
           05 FILLER                             PIC    X(240)    VALUE
               "Quitter".
           05 FILLER                             PIC    X(240)    VALUE
               "終了".
           05 FILLER                             PIC    X(240)    VALUE
               "退出".
      *> Text 4: Player name
           05 FILLER                             PIC    X(240)    VALUE
               "Player name".
           05 FILLER                             PIC    X(240)    VALUE
               "Nome do jogador".
           05 FILLER                             PIC    X(240)    VALUE
               "Nombre del jugador".
           05 FILLER                             PIC    X(240)    VALUE
               "Nom du joueur".
           05 FILLER                             PIC    X(240)    VALUE
               "プレイヤー名".
           05 FILLER                             PIC    X(240)    VALUE
               "玩家名称".
      *> Text 5: Game mode
           05 FILLER                             PIC    X(240)    VALUE
               "Game mode".
           05 FILLER                             PIC    X(240)    VALUE
               "Modo de jogo".
           05 FILLER                             PIC    X(240)    VALUE
               "Modo de juego".
           05 FILLER                             PIC    X(240)    VALUE
               "Mode de jeu".
           05 FILLER                             PIC    X(240)    VALUE
               "ゲームモード".
           05 FILLER                             PIC    X(240)    VALUE
               "游戏模式".
      *> Text 6: Language
           05 FILLER                             PIC    X(240)    VALUE
               "Language".
           05 FILLER                             PIC    X(240)    VALUE
               "Idioma".
           05 FILLER                             PIC    X(240)    VALUE
               "Idioma".
           05 FILLER                             PIC    X(240)    VALUE
               "Langue".
           05 FILLER                             PIC    X(240)    VALUE
               "言語".
           05 FILLER                             PIC    X(240)    VALUE
               "语言".
      *> Text 7: Survival
           05 FILLER                             PIC    X(240)    VALUE
               "Survival".
           05 FILLER                             PIC    X(240)    VALUE
               "Sobrevivência".
           05 FILLER                             PIC    X(240)    VALUE
               "Supervivencia".
           05 FILLER                             PIC    X(240)    VALUE
               "Survie".
           05 FILLER                             PIC    X(240)    VALUE
               "サバイバル".
           05 FILLER                             PIC    X(240)    VALUE
               "生存".
      *> Text 8: Creative
           05 FILLER                             PIC    X(240)    VALUE
               "Creative".
           05 FILLER                             PIC    X(240)    VALUE
               "Criativo".
           05 FILLER                             PIC    X(240)    VALUE
               "Creativo".
           05 FILLER                             PIC    X(240)    VALUE
               "Créatif".
           05 FILLER                             PIC    X(240)    VALUE
               "クリエイティブ".
           05 FILLER                             PIC    X(240)    VALUE
               "创造".
      *> Text 9: Adventure
           05 FILLER                             PIC    X(240)    VALUE
               "Adventure".
           05 FILLER                             PIC    X(240)    VALUE
               "Aventura".
           05 FILLER                             PIC    X(240)    VALUE
               "Aventura".
           05 FILLER                             PIC    X(240)    VALUE
               "Aventure".
           05 FILLER                             PIC    X(240)    VALUE
               "アドベンチャー".
           05 FILLER                             PIC    X(240)    VALUE
               "冒险".
      *> Text 10: Spectator
           05 FILLER                             PIC    X(240)    VALUE
               "Spectator".
           05 FILLER                             PIC    X(240)    VALUE
               "Espectador".
           05 FILLER                             PIC    X(240)    VALUE
               "Espectador".
           05 FILLER                             PIC    X(240)    VALUE
               "Spectateur".
           05 FILLER                             PIC    X(240)    VALUE
               "スペクテイター".
           05 FILLER                             PIC    X(240)    VALUE
               "旁观".
      *> Text 11: A flat voxel world in the spirit of CobolCraft, written in RustCOBOL
           05 FILLER                             PIC    X(240)    VALUE
               "A flat voxel world in the spirit of CobolCraft, written in RustCOBOL".
           05 FILLER                             PIC    X(240)    VALUE
               "Um mundo plano de blocos no espírito do CobolCraft, escrito em RustCOBOL".
           05 FILLER                             PIC    X(240)    VALUE
               "Un mundo plano de bloques al estilo de CobolCraft, escrito en RustCOBOL".
           05 FILLER                             PIC    X(240)    VALUE
               "Un monde plat de blocs dans l'esprit de CobolCraft, écrit en RustCOBOL".
           05 FILLER                             PIC    X(240)    VALUE
               "CobolCraft の精神を受け継ぐ平らなブロックの世界。RustCOBOL で書かれています".
           05 FILLER                             PIC    X(240)    VALUE
               "秉承 CobolCraft 精神的平坦方块世界，用 RustCOBOL 编写".
      *> Text 12: Credits
           05 FILLER                             PIC    X(240)    VALUE
               "Credits".
           05 FILLER                             PIC    X(240)    VALUE
               "Créditos".
           05 FILLER                             PIC    X(240)    VALUE
               "Créditos".
           05 FILLER                             PIC    X(240)    VALUE
               "Crédits".
           05 FILLER                             PIC    X(240)    VALUE
               "クレジット".
           05 FILLER                             PIC    X(240)    VALUE
               "致谢".
      *> Text 13: Based on CobolCraft by Fabian Meyer (meyfa), a Minecraft server written in COBOL
           05 FILLER                             PIC    X(240)    VALUE
               "Based on CobolCraft by Fabian Meyer (meyfa), a Minecraft server written in COBOL.".
           05 FILLER                             PIC    X(240)    VALUE
               "Baseado no CobolCraft de Fabian Meyer (meyfa), um servidor de Minecraft escrito em COBOL.".
           05 FILLER                             PIC    X(240)    VALUE
               "Basado en CobolCraft de Fabian Meyer (meyfa), un servidor de Minecraft escrito en COBOL.".
           05 FILLER                             PIC    X(240)    VALUE
               "D'après CobolCraft de Fabian Meyer (meyfa), un serveur Minecraft écrit en COBOL.".
           05 FILLER                             PIC    X(240)    VALUE
               "Fabian Meyer (meyfa) が COBOL で書いた Minecraft サーバー、CobolCraft に基づいています。".
           05 FILLER                             PIC    X(240)    VALUE
               "基于 Fabian Meyer (meyfa) 用 COBOL 编写的 Minecraft 服务器 CobolCraft。".
      *> Text 14: CobolCraft is released under the MIT License. Copyright (c) 2024 - 2025 Fabian M
           05 FILLER                             PIC    X(240)    VALUE
               "CobolCraft is released under the MIT License. Copyright (c) 2024 - 2025 Fabian Meyer.".
           05 FILLER                             PIC    X(240)    VALUE
               "O CobolCraft é distribuído sob a Licença MIT. Copyright (c) 2024 - 2025 Fabian Meyer.".
           05 FILLER                             PIC    X(240)    VALUE
               "CobolCraft se distribuye bajo la Licencia MIT. Copyright (c) 2024 - 2025 Fabian Meyer.".
           05 FILLER                             PIC    X(240)    VALUE
               "CobolCraft est distribué sous licence MIT. Copyright (c) 2024 - 2025 Fabian Meyer.".
           05 FILLER                             PIC    X(240)    VALUE
               "CobolCraft は MIT ライセンスで公開されています。Copyright (c) 2024 - 2025 Fabian Meyer.".
           05 FILLER                             PIC    X(240)    VALUE
               "CobolCraft 以 MIT 许可证发布。Copyright (c) 2024 - 2025 Fabian Meyer.".
      *> Text 15: Minecraft is a trademark of Mojang Synergies AB. Neither CobolCraft nor PowerCob
           05 FILLER                             PIC    X(240)    VALUE
               "Minecraft is a trademark of Mojang Synergies AB. Neither CobolCraft nor PowerCobolCraft is affiliated with or endorsed by Mojang.".
           05 FILLER                             PIC    X(240)    VALUE
               "Minecraft é uma marca da Mojang Synergies AB. Nem o CobolCraft nem o PowerCobolCraft têm vínculo com a Mojang ou o seu aval.".
           05 FILLER                             PIC    X(240)    VALUE
               "Minecraft es una marca de Mojang Synergies AB. Ni CobolCraft ni PowerCobolCraft están afiliados a Mojang ni cuentan con su aval.".
           05 FILLER                             PIC    X(240)    VALUE
               "Minecraft est une marque de Mojang Synergies AB. Ni CobolCraft ni PowerCobolCraft ne sont affiliés à Mojang ni approuvés par elle.".
           05 FILLER                             PIC    X(240)    VALUE
               "Minecraft は Mojang Synergies AB の商標です。CobolCraft と PowerCobolCraft は Mojang と提携しておらず、承認も受けていません。".
           05 FILLER                             PIC    X(240)    VALUE
               "Minecraft 是 Mojang Synergies AB 的商标。CobolCraft 和 PowerCobolCraft 均未与 Mojang 合作，也未获其认可。".
      *> Text 16: Every texture is drawn by this program as it starts and every sound is synthesiz
           05 FILLER                             PIC    X(240)    VALUE
               "Every texture is drawn by this program as it starts and every sound is synthesized: no Minecraft assets are used.".
           05 FILLER                             PIC    X(240)    VALUE
               "Cada textura é desenhada por este programa ao iniciar e cada som é sintetizado: nenhum recurso do Minecraft é usado.".
           05 FILLER                             PIC    X(240)    VALUE
               "Cada textura la dibuja este programa al iniciar y cada sonido es sintetizado: no se usa ningún recurso de Minecraft.".
           05 FILLER                             PIC    X(240)    VALUE
               "Chaque texture est dessinée par ce programme au démarrage et chaque son est synthétisé : aucune ressource de Minecraft n'est utilisée.".
           05 FILLER                             PIC    X(240)    VALUE
               "テクスチャはすべて起動時にこのプログラムが描き、音はすべて合成したものです。Minecraft の素材は使っていません。".
           05 FILLER                             PIC    X(240)    VALUE
               "所有纹理都由本程序在启动时绘制，所有声音均为合成：未使用任何 Minecraft 素材。".
      *> Text 17: Built with
           05 FILLER                             PIC    X(240)    VALUE
               "Built with".
           05 FILLER                             PIC    X(240)    VALUE
               "Feito com".
           05 FILLER                             PIC    X(240)    VALUE
               "Hecho con".
           05 FILLER                             PIC    X(240)    VALUE
               "Conçu avec".
           05 FILLER                             PIC    X(240)    VALUE
               "制作:".
           05 FILLER                             PIC    X(240)    VALUE
               "构建工具：".
      *> Text 18: Server console
           05 FILLER                             PIC    X(240)    VALUE
               "Server console".
           05 FILLER                             PIC    X(240)    VALUE
               "Console do servidor".
           05 FILLER                             PIC    X(240)    VALUE
               "Consola del servidor".
           05 FILLER                             PIC    X(240)    VALUE
               "Console du serveur".
           05 FILLER                             PIC    X(240)    VALUE
               "サーバーコンソール".
           05 FILLER                             PIC    X(240)    VALUE
               "服务器控制台".
      *> Text 19: A command (try help) or a chat message, then Enter
           05 FILLER                             PIC    X(240)    VALUE
               "A command (try help) or a chat message, then Enter".
           05 FILLER                             PIC    X(240)    VALUE
               "Um comando (experimente help) ou uma mensagem, depois Enter".
           05 FILLER                             PIC    X(240)    VALUE
               "Un comando (pruebe help) o un mensaje, luego Enter".
           05 FILLER                             PIC    X(240)    VALUE
               "Une commande (essayez help) ou un message, puis Entrée".
           05 FILLER                             PIC    X(240)    VALUE
               "コマンド（help など）かチャットを入力して Enter".
           05 FILLER                             PIC    X(240)    VALUE
               "输入命令（试试 help）或聊天消息，然后按 Enter".
      *> Text 20: Game menu
           05 FILLER                             PIC    X(240)    VALUE
               "Game menu".
           05 FILLER                             PIC    X(240)    VALUE
               "Menu do jogo".
           05 FILLER                             PIC    X(240)    VALUE
               "Menú del juego".
           05 FILLER                             PIC    X(240)    VALUE
               "Menu du jeu".
           05 FILLER                             PIC    X(240)    VALUE
               "ゲームメニュー".
           05 FILLER                             PIC    X(240)    VALUE
               "游戏菜单".
      *> Text 21: Back to game
           05 FILLER                             PIC    X(240)    VALUE
               "Back to game".
           05 FILLER                             PIC    X(240)    VALUE
               "Voltar ao jogo".
           05 FILLER                             PIC    X(240)    VALUE
               "Volver al juego".
           05 FILLER                             PIC    X(240)    VALUE
               "Retour au jeu".
           05 FILLER                             PIC    X(240)    VALUE
               "ゲームに戻る".
           05 FILLER                             PIC    X(240)    VALUE
               "返回游戏".
      *> Text 22: Save world
           05 FILLER                             PIC    X(240)    VALUE
               "Save world".
           05 FILLER                             PIC    X(240)    VALUE
               "Salvar o mundo".
           05 FILLER                             PIC    X(240)    VALUE
               "Guardar el mundo".
           05 FILLER                             PIC    X(240)    VALUE
               "Enregistrer le monde".
           05 FILLER                             PIC    X(240)    VALUE
               "ワールドを保存".
           05 FILLER                             PIC    X(240)    VALUE
               "保存世界".
      *> Text 23: Save and quit to title
           05 FILLER                             PIC    X(240)    VALUE
               "Save and quit to title".
           05 FILLER                             PIC    X(240)    VALUE
               "Salvar e voltar ao título".
           05 FILLER                             PIC    X(240)    VALUE
               "Guardar y volver al título".
           05 FILLER                             PIC    X(240)    VALUE
               "Enregistrer et revenir au titre".
           05 FILLER                             PIC    X(240)    VALUE
               "保存してタイトルへ".
           05 FILLER                             PIC    X(240)    VALUE
               "保存并返回标题".
      *> Text 24: You died!
           05 FILLER                             PIC    X(240)    VALUE
               "You died!".
           05 FILLER                             PIC    X(240)    VALUE
               "Você morreu!".
           05 FILLER                             PIC    X(240)    VALUE
               "¡Has muerto!".
           05 FILLER                             PIC    X(240)    VALUE
               "Vous êtes mort !".
           05 FILLER                             PIC    X(240)    VALUE
               "死んでしまった！".
           05 FILLER                             PIC    X(240)    VALUE
               "你死了！".
      *> Text 25: Respawn
           05 FILLER                             PIC    X(240)    VALUE
               "Respawn".
           05 FILLER                             PIC    X(240)    VALUE
               "Renascer".
           05 FILLER                             PIC    X(240)    VALUE
               "Reaparecer".
           05 FILLER                             PIC    X(240)    VALUE
               "Réapparaître".
           05 FILLER                             PIC    X(240)    VALUE
               "リスポーン".
           05 FILLER                             PIC    X(240)    VALUE
               "重生".
      *> Text 26: W A S D walk · Space jump · Mouse look · Left click break · Right click place ·
           05 FILLER                             PIC    X(240)    VALUE
               "W A S D walk · Space jump · Mouse look · Left click break · Right click place · 1 to 9 pick a block · T console · Esc menu".
           05 FILLER                             PIC    X(240)    VALUE
               "W A S D andar · Espaço pular · Mouse olhar · Clique esquerdo quebrar · Clique direito colocar · 1 a 9 escolher bloco · T console · Esc menu".
           05 FILLER                             PIC    X(240)    VALUE
               "W A S D caminar · Espacio saltar · Ratón mirar · Clic izquierdo romper · Clic derecho colocar · 1 a 9 elegir bloque · T consola · Esc menú".
           05 FILLER                             PIC    X(240)    VALUE
               "W A S D marcher · Espace sauter · Souris regarder · Clic gauche casser · Clic droit poser · 1 à 9 choisir un bloc · T console · Échap menu".
           05 FILLER                             PIC    X(240)    VALUE
               "W A S D 移動 · Space ジャンプ · マウス 視点 · 左クリック 破壊 · 右クリック 設置 · 1〜9 ブロック選択 · T コンソール · Esc メニュー".
           05 FILLER                             PIC    X(240)    VALUE
               "W A S D 移动 · 空格 跳跃 · 鼠标 视角 · 左键 破坏 · 右键 放置 · 1–9 选择方块 · T 控制台 · Esc 菜单".
      *> Text 27: Click the world to play
           05 FILLER                             PIC    X(240)    VALUE
               "Click the world to play".
           05 FILLER                             PIC    X(240)    VALUE
               "Clique no mundo para jogar".
           05 FILLER                             PIC    X(240)    VALUE
               "Haga clic en el mundo para jugar".
           05 FILLER                             PIC    X(240)    VALUE
               "Cliquez sur le monde pour jouer".
           05 FILLER                             PIC    X(240)    VALUE
               "ワールドをクリックしてプレイ".
           05 FILLER                             PIC    X(240)    VALUE
               "点击世界开始游戏".
      *> Text 28: Day
           05 FILLER                             PIC    X(240)    VALUE
               "Day".
           05 FILLER                             PIC    X(240)    VALUE
               "Dia".
           05 FILLER                             PIC    X(240)    VALUE
               "Día".
           05 FILLER                             PIC    X(240)    VALUE
               "Jour".
           05 FILLER                             PIC    X(240)    VALUE
               "日数".
           05 FILLER                             PIC    X(240)    VALUE
               "天数".
      *> Text 29: Send
           05 FILLER                             PIC    X(240)    VALUE
               "Send".
           05 FILLER                             PIC    X(240)    VALUE
               "Enviar".
           05 FILLER                             PIC    X(240)    VALUE
               "Enviar".
           05 FILLER                             PIC    X(240)    VALUE
               "Envoyer".
           05 FILLER                             PIC    X(240)    VALUE
               "送信".
           05 FILLER                             PIC    X(240)    VALUE
               "发送".
      *> Text 30: Close
           05 FILLER                             PIC    X(240)    VALUE
               "Close".
           05 FILLER                             PIC    X(240)    VALUE
               "Fechar".
           05 FILLER                             PIC    X(240)    VALUE
               "Cerrar".
           05 FILLER                             PIC    X(240)    VALUE
               "Fermer".
           05 FILLER                             PIC    X(240)    VALUE
               "閉じる".
           05 FILLER                             PIC    X(240)    VALUE
               "关闭".
      *> Text 31: Time
           05 FILLER                             PIC    X(240)    VALUE
               "Time".
           05 FILLER                             PIC    X(240)    VALUE
               "Hora".
           05 FILLER                             PIC    X(240)    VALUE
               "Hora".
           05 FILLER                             PIC    X(240)    VALUE
               "Heure".
           05 FILLER                             PIC    X(240)    VALUE
               "時刻".
           05 FILLER                             PIC    X(240)    VALUE
               "时间".
      *> Text 32: Message
           05 FILLER                             PIC    X(240)    VALUE
               "Message".
           05 FILLER                             PIC    X(240)    VALUE
               "Mensagem".
           05 FILLER                             PIC    X(240)    VALUE
               "Mensaje".
           05 FILLER                             PIC    X(240)    VALUE
               "Message".
           05 FILLER                             PIC    X(240)    VALUE
               "メッセージ".
           05 FILLER                             PIC    X(240)    VALUE
               "消息".
      *> Text 33: Flying
           05 FILLER                             PIC    X(240)    VALUE
               "Flying".
           05 FILLER                             PIC    X(240)    VALUE
               "Voando".
           05 FILLER                             PIC    X(240)    VALUE
               "Volando".
           05 FILLER                             PIC    X(240)    VALUE
               "En vol".
           05 FILLER                             PIC    X(240)    VALUE
               "飛行中".
           05 FILLER                             PIC    X(240)    VALUE
               "飞行中".
       01 WS-TEXT-TABLE REDEFINES WS-TEXT-DATA.
           05 WS-TEXT-ROW                        OCCURS 33.
               10 WS-TEXT-CELL                   PIC    X(240)    OCCURS 6.

       LINKAGE SECTION.
       01 LK-LANGUAGE                            PIC    9.
       01 LK-TEXT-ID                             PIC    99.
       01 LK-TEXT                                PIC    X(240).

       PROCEDURE DIVISION USING LK-LANGUAGE LK-TEXT-ID LK-TEXT.

           *> A language or a text number out of range answers the English text, or spaces: a caption
           *> is never left holding another text's words.

           IF    LK-TEXT-ID < 1 OR LK-TEXT-ID > 33
               SET     LK-TEXT                       TO    SPACES
               GOBACK
           END-IF
           IF    LK-LANGUAGE < 1 OR LK-LANGUAGE > 6
               SET     LK-TEXT                       TO    WS-TEXT-CELL(LK-TEXT-ID 1)
           ELSE
               SET     LK-TEXT                       TO    WS-TEXT-CELL(LK-TEXT-ID LK-LANGUAGE)
           END-IF
           GOBACK.
