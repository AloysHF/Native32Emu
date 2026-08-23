// ============================================================
// Native32Emu — Landing Page Scripts
// i18n + Carousel Gallery + Animations
// ============================================================

(function () {
  'use strict';

  // ================================================================
  // GAME DATA — All 84 Native32 games
  // ================================================================
  var GAMES = [
    // EACT — Action Games (11)
    { cat: 'EACT', zh: '赤刃', en: 'Bloody Blade', img: 'docs/images/EACT/EBBLADE.png', descZh: '经典横版动作过关游戏，女刺客的复仇之旅', descEn: 'Classic side-scrolling action game — a female assassin\'s journey of revenge' },
    { cat: 'EACT', zh: '枪火', en: 'Gun Fire', img: 'docs/images/EACT/EGUNFIRE.png', descZh: '横版射击游戏，打击黑社会对抗独裁者', descEn: 'Side-scrolling shooter — fight the syndicate and take down dictators' },
    { cat: 'EACT', zh: '钢铁风暴', en: 'Metal Storm', img: 'docs/images/EACT/EMETAL.png', descZh: '四向自由卷轴射击，近未来军事科技', descEn: 'Four-directional free-scroll shooter set in a near-future world' },
    { cat: 'EACT', zh: '海盗', en: 'Pirate', img: 'docs/images/EACT/Epirate.png', descZh: '以炸弹人为原型的休闲桌面游戏', descEn: 'Bomberman-inspired casual board game' },
    { cat: 'EACT', zh: '风暴之翼', en: 'Storm', img: 'docs/images/EACT/ESTORM.png', descZh: '纵版射击，王牌飞行员深入敌后侦察', descEn: 'Vertical shooter — ace pilots recon deep behind enemy lines' },
    { cat: 'EACT', zh: '小小我', en: 'Little Me', img: 'docs/images/EACT/LittleMe.png', descZh: '水下主题冒险游戏，控制小美人鱼冒险', descEn: 'Underwater adventure — guide a little mermaid through the deep' },
    { cat: 'EACT', zh: '失落之剑', en: 'Lost Sword', img: 'docs/images/EACT/LostSwor.png', descZh: '剑与魔法的奇幻动作冒险', descEn: 'Fantasy action adventure of swords and magic' },
    { cat: 'EACT', zh: '鼓舞', en: 'Music Game', img: 'docs/images/EACT/MusicGam.png', descZh: '音乐游戏，跟随节奏按下方向键', descEn: 'Rhythm music game — hit the arrow keys to the beat' },
    { cat: 'EACT', zh: '泡泡精灵', en: 'PoPo Fun', img: 'docs/images/EACT/PoPoFun.png', descZh: '经典踩气球游戏', descEn: 'Classic balloon-stomping game' },
    { cat: 'EACT', zh: '少林功夫', en: 'ShaoLin Kung Fu', img: 'docs/images/EACT/ShaoLinK.png', descZh: '少林功夫横版格斗，学习武术招式闯关', descEn: 'Shaolin kung fu side-scrolling fighter — learn martial arts to progress' },
    { cat: 'EACT', zh: '三只小猪', en: 'Three Pigs', img: 'docs/images/EACT/ThreePig.png', descZh: '射箭防守游戏，抵御大灰狼的进攻', descEn: 'Archery defense game — protect the pigs from the big bad wolf' },

    // EELA — Educational Games (32)
    { cat: 'EELA', zh: '加 21', en: 'Adding 21', img: 'docs/images/EELA/Adding21.png', descZh: '数学加法练习游戏', descEn: 'Math addition practice game' },
    { cat: 'EELA', zh: '字母排序', en: 'Alphabetical Order', img: 'docs/images/EELA/AlpOrder.png', descZh: '字母排序学习游戏', descEn: 'Alphabet ordering learning game' },
    { cat: 'EELA', zh: '动物朋友', en: 'Animal Friends', img: 'docs/images/EELA/AnimalFr.png', descZh: '动物认知学习游戏', descEn: 'Animal recognition learning game' },
    { cat: 'EELA', zh: '动物一', en: 'Animals 1', img: 'docs/images/EELA/Animals1.png', descZh: '通过图片认识各种动物', descEn: 'Learn about animals through pictures' },
    { cat: 'EELA', zh: '动物二', en: 'Animals 2', img: 'docs/images/EELA/Animals2.png', descZh: '认识更多种类的动物', descEn: 'Discover even more kinds of animals' },
    { cat: 'EELA', zh: '小鸡回家', en: 'Chicklin', img: 'docs/images/EELA/Chicklin.png', descZh: '判断数学等式是否正确', descEn: 'Judge whether math equations are correct' },
    { cat: 'EELA', zh: '颜色魔法', en: 'Colors Magic', img: 'docs/images/EELA/ColorsMa.png', descZh: '颜色配对学习游戏', descEn: 'Color matching learning game' },
    { cat: 'EELA', zh: '阅读理解', en: 'Comprehension', img: 'docs/images/EELA/Comprehe.png', descZh: '阅读理解练习游戏', descEn: 'Reading comprehension practice' },
    { cat: 'EELA', zh: '数字猎人', en: 'Digital Hunt', img: 'docs/images/EELA/DigiHunt.png', descZh: '在星空背景下寻找并收集数字', descEn: 'Hunt and collect numbers against a starry backdrop' },
    { cat: 'EELA', zh: '找匹配', en: 'Find The Match', img: 'docs/images/EELA/FindTheM.png', descZh: '数量比较游戏', descEn: 'Quantity comparison game' },
    { cat: 'EELA', zh: '水果一', en: 'Fruits 1', img: 'docs/images/EELA/Fruits1.png', descZh: '通过趣味互动认识各种水果', descEn: 'Learn about fruits through fun interactions' },
    { cat: 'EELA', zh: '水果二', en: 'Fruits 2', img: 'docs/images/EELA/Fruits2.png', descZh: '认识更多种类的水果', descEn: 'Discover even more kinds of fruits' },
    { cat: 'EELA', zh: '地理', en: 'Geography', img: 'docs/images/EELA/Geograph.png', descZh: '互动学习世界地理知识', descEn: 'Interactive world geography learning' },
    { cat: 'EELA', zh: '生物', en: 'Living Things', img: 'docs/images/EELA/LivingTi.png', descZh: '认识各种生物及其特征', descEn: 'Learn about living things and their traits' },
    { cat: 'EELA', zh: '魔法链', en: 'Magic Chain', img: 'docs/images/EELA/MagicCha.png', descZh: '通过魔法链条连接学习字母组合', descEn: 'Learn letter combinations through magic chains' },
    { cat: 'EELA', zh: '魔法 A', en: 'Magical A', img: 'docs/images/EELA/MagicalA.png', descZh: '通过魔法主题学习字母 A', descEn: 'Learn the letter A through a magical theme' },
    { cat: 'EELA', zh: '数学讨价还价', en: 'Math Bargain', img: 'docs/images/EELA/MathBarg.png', descZh: '在小船上进行数学计算挑战', descEn: 'Math calculation challenge on a little boat' },
    { cat: 'EELA', zh: '猴子军队', en: 'Monkey Army', img: 'docs/images/EELA/MonkeyAr.png', descZh: '跟随小猴子进行趣味数学练习', descEn: 'Fun math practice with a little monkey' },
    { cat: 'EELA', zh: '比大小', en: 'More Or Less', img: 'docs/images/EELA/MoreOrLe.png', descZh: '判断两边宝石数量', descEn: 'Compare quantities of gems on each side' },
    { cat: 'EELA', zh: '音乐基础', en: 'Music Basic', img: 'docs/images/EELA/MusicBas.png', descZh: '认识音符和节奏', descEn: 'Learn about notes and rhythm' },
    { cat: 'EELA', zh: '单词拼块', en: 'Ordered Blocks', img: 'docs/images/EELA/OrdBlock.png', descZh: '英语单词识别和拼写游戏', descEn: 'English word recognition and spelling game' },
    { cat: 'EELA', zh: '重新拼字', en: 'Re-Letter', img: 'docs/images/EELA/ReLetter.png', descZh: '通过火焰场景学习辨识字母', descEn: 'Learn to identify letters in a fiery scene' },
    { cat: 'EELA', zh: '看图说话', en: 'Reads Picture', img: 'docs/images/EELA/ReadsPic.png', descZh: '通过图片学习词汇和表达', descEn: 'Learn vocabulary through pictures' },
    { cat: 'EELA', zh: '汉语学堂一', en: 'School 1', img: 'docs/images/EELA/SCHOOL1.png', descZh: '中英文双语学习，全 3D 图画', descEn: 'Bilingual Chinese-English learning with full 3D graphics' },
    { cat: 'EELA', zh: '汉语学堂二', en: 'School 2', img: 'docs/images/EELA/SCHOOL2.png', descZh: '中英文双语学习，配以双重语音', descEn: 'Bilingual Chinese-English learning with dual audio' },
    { cat: 'EELA', zh: '搜索者', en: 'Seeker', img: 'docs/images/EELA/Seeker.png', descZh: '搜索寻找类益智游戏', descEn: 'Search and find puzzle game' },
    { cat: 'EELA', zh: '简单算术', en: 'Simple Arithmetic', img: 'docs/images/EELA/SimpleAr.png', descZh: '数学基础加减乘除运算练习', descEn: 'Basic arithmetic practice — add, subtract, multiply, divide' },
    { cat: 'EELA', zh: '快速算术', en: 'Speed Arithmetic', img: 'docs/images/EELA/SpeedAri.png', descZh: '限时完成数学计算挑战', descEn: 'Timed math calculation challenge' },
    { cat: 'EELA', zh: '超级加法', en: 'Super Add', img: 'docs/images/EELA/SuperAdd.png', descZh: '通过爱心气球场景进行加法训练', descEn: 'Addition training with heart balloon scenes' },
    { cat: 'EELA', zh: '我们的时间', en: 'Us Time', img: 'docs/images/EELA/UsTime.png', descZh: '学习看钟表和理解时间概念', descEn: 'Learn to read clocks and understand time' },
    { cat: 'EELA', zh: '单词选择', en: 'Word Choice', img: 'docs/images/EELA/WordChoi.png', descZh: '通过选择正确单词练习词汇', descEn: 'Practice vocabulary by choosing the correct word' },
    { cat: 'EELA', zh: '单词车间', en: 'Workshop', img: 'docs/images/EELA/Workshop.png', descZh: '英语单词拼写练习', descEn: 'English word spelling practice' },

    // EPOP — Featured Games (9)
    { cat: 'EPOP', zh: '赤刃', en: 'Bloody Blade', img: 'docs/images/EPOP/EBBLADE.png', descZh: '经典横版动作过关游戏', descEn: 'Classic side-scrolling action game' },
    { cat: 'EPOP', zh: '极速任务', en: 'Express', img: 'docs/images/EPOP/EExpress.png', descZh: '类赛车多角色游戏，多种赛道场景', descEn: 'Racing-style multi-character game with various track scenes' },
    { cat: 'EPOP', zh: '枪火', en: 'Gun Fire', img: 'docs/images/EPOP/EGUNFIRE.png', descZh: '横版射击游戏', descEn: 'Side-scrolling shooter' },
    { cat: 'EPOP', zh: '钢铁风暴', en: 'Metal Storm', img: 'docs/images/EPOP/EMETAL.png', descZh: '四向自由卷轴射击', descEn: 'Four-directional free-scroll shooter' },
    { cat: 'EPOP', zh: '海盗', en: 'Pirate', img: 'docs/images/EPOP/Epirate.png', descZh: '以炸弹人为原型的休闲游戏', descEn: 'Bomberman-inspired casual game' },
    { cat: 'EPOP', zh: '符文之语', en: 'Rune Word', img: 'docs/images/EPOP/ERuneWod.png', descZh: '魔法学徒的符文对对碰修炼', descEn: 'A magical apprentice\'s rune-matching training' },
    { cat: 'EPOP', zh: '风暴之翼', en: 'Storm', img: 'docs/images/EPOP/ESTORM.png', descZh: '纵版射击游戏', descEn: 'Vertical shooter' },
    { cat: 'EPOP', zh: '汉语学堂一', en: 'School 1', img: 'docs/images/EPOP/SCHOOL1.png', descZh: '中英文双语学习', descEn: 'Bilingual Chinese-English learning' },
    { cat: 'EPOP', zh: '汉语学堂二', en: 'School 2', img: 'docs/images/EPOP/SCHOOL2.png', descZh: '中英文双语学习', descEn: 'Bilingual Chinese-English learning' },

    // EPUZ — Puzzle Games (24)
    { cat: 'EPUZ', zh: '坏男孩', en: 'Bad Boy', img: 'docs/images/EPUZ/BadBoy.png', descZh: '篮球投篮游戏', descEn: 'Basketball shooting game' },
    { cat: 'EPUZ', zh: '铃铛女孩', en: 'Bell Girls', img: 'docs/images/EPUZ/BellGirl.png', descZh: '控制角色收集铃铛', descEn: 'Guide the character to collect bells' },
    { cat: 'EPUZ', zh: '小猫快跑', en: 'Cat Run', img: 'docs/images/EPUZ/CatRun.png', descZh: '选择小猫参加跑步比赛', descEn: 'Pick a kitty and race' },
    { cat: 'EPUZ', zh: 'CE 城堡', en: 'CE Castle', img: 'docs/images/EPUZ/CeCastle.png', descZh: '天空城堡主题益智游戏', descEn: 'Sky castle themed puzzle game' },
    { cat: 'EPUZ', zh: '龙', en: 'Dragon', img: 'docs/images/EPUZ/Dragon.png', descZh: '龙主题益智闯关游戏', descEn: 'Dragon themed puzzle adventure' },
    { cat: 'EPUZ', zh: '仙女博士', en: 'Dr. Fairy', img: 'docs/images/EPUZ/DrFairy.png', descZh: '梦幻仙子城镇主题益智游戏', descEn: 'Dreamy fairy town themed puzzle game' },
    { cat: 'EPUZ', zh: '元素大冒险', en: 'Element', img: 'docs/images/EPUZ/Element.png', descZh: '俄罗斯方块原型游戏', descEn: 'Tetris-inspired block puzzle game' },
    { cat: 'EPUZ', zh: '符文之语', en: 'Rune Word', img: 'docs/images/EPUZ/ERuneWod.png', descZh: '符文对对碰游戏', descEn: 'Rune matching game' },
    { cat: 'EPUZ', zh: '食物雨', en: 'Food Rain', img: 'docs/images/EPUZ/FoodRain.png', descZh: '接住从天而降的各种食物', descEn: 'Catch the falling food from the sky' },
    { cat: 'EPUZ', zh: '青蛙', en: 'Frog', img: 'docs/images/EPUZ/Frog.png', descZh: '青蛙跳跃益智游戏', descEn: 'Frog jumping puzzle game' },
    { cat: 'EPUZ', zh: '水果派对', en: 'Fruit Party', img: 'docs/images/EPUZ/FruParty.png', descZh: '水果配对消除游戏', descEn: 'Fruit matching elimination game' },
    { cat: 'EPUZ', zh: '宝石森林', en: 'Gem Woods', img: 'docs/images/EPUZ/GemWoods.png', descZh: '在森林中收集和匹配宝石', descEn: 'Collect and match gems in the forest' },
    { cat: 'EPUZ', zh: '翻翻乐', en: 'Guess', img: 'docs/images/EPUZ/Guess.png', descZh: '纸牌翻牌记忆游戏', descEn: 'Card-flipping memory game' },
    { cat: 'EPUZ', zh: '躲猫猫', en: 'Hide & Seek', img: 'docs/images/EPUZ/HideSeek.png', descZh: '寻找隐藏的角色和物品', descEn: 'Find hidden characters and objects' },
    { cat: 'EPUZ', zh: '幸运兔', en: 'Lucky Rabbits', img: 'docs/images/EPUZ/LRabbits.png', descZh: '控制小兔子进行冒险', descEn: 'Guide little rabbits on an adventure' },
    { cat: 'EPUZ', zh: '打地鼠', en: 'Mouse', img: 'docs/images/EPUZ/Mouse.png', descZh: '经典打地鼠游戏', descEn: 'Classic whack-a-mole game' },
    { cat: 'EPUZ', zh: '木偶', en: 'Mushu Mus', img: 'docs/images/EPUZ/MushuMus.png', descZh: '木偶射击益智游戏', descEn: 'Puppet shooting puzzle game' },
    { cat: 'EPUZ', zh: '猴子', en: 'Nau Orang', img: 'docs/images/EPUZ/NauOrang.png', descZh: '调皮橘子益智游戏', descEn: 'Naughty orange puzzle game' },
    { cat: 'EPUZ', zh: '果园', en: 'Orchard', img: 'docs/images/EPUZ/Orchard.png', descZh: '果园水果分类游戏', descEn: 'Orchard fruit sorting game' },
    { cat: 'EPUZ', zh: '海盗 C', en: 'Pirate C', img: 'docs/images/EPUZ/PirateC.png', descZh: '驾驶海盗船进行海上冒险', descEn: 'Sail a pirate ship on a sea adventure' },
    { cat: 'EPUZ', zh: '小熊拼图', en: 'Puzzle', img: 'docs/images/EPUZ/Puzzle.png', descZh: '经典拼图游戏', descEn: 'Classic jigsaw puzzle game' },
    { cat: 'EPUZ', zh: '星空吞吞', en: 'Snake Mania', img: 'docs/images/EPUZ/SnakeMa.png', descZh: '贪吃蛇游戏', descEn: 'Classic snake game' },
    { cat: 'EPUZ', zh: '数独', en: 'Sudoku', img: 'docs/images/EPUZ/Sudoku.png', descZh: '数独益智游戏', descEn: 'Sudoku puzzle game' },
    { cat: 'EPUZ', zh: '零猎人', en: 'Zero Hunt', img: 'docs/images/EPUZ/ZeroHunt.png', descZh: '在科幻场景中寻找数字零', descEn: 'Hunt for the number zero in a sci-fi scene' },

    // ESPG — Sport Games (3)
    { cat: 'ESPG', zh: '篮球', en: 'Basketball', img: 'docs/images/ESPG/Basketba.png', descZh: '篮球投篮竞技', descEn: 'Basketball shooting competition' },
    { cat: 'ESPG', zh: '保龄球', en: 'Bowling', img: 'docs/images/ESPG/Bowling.png', descZh: '保龄球运动', descEn: 'Bowling sport game' },
    { cat: 'ESPG', zh: '极速任务', en: 'Express', img: 'docs/images/ESPG/EExpress.png', descZh: '类赛车多角色游戏', descEn: 'Racing-style multi-character game' },

    // ETAB — Board Games (4)
    { cat: 'ETAB', zh: '幸运 21', en: 'Lucky 21', img: 'docs/images/ETAB/Lucky21.png', descZh: '21 点赌博纸牌游戏', descEn: 'Blackjack card game' },
    { cat: 'ETAB', zh: '幸运宝盒', en: 'Lucky Box', img: 'docs/images/ETAB/LuckyBox.png', descZh: '苹果机、梭哈、赌骰子合集', descEn: 'Slot machine, poker & dice game collection' },
    { cat: 'ETAB', zh: '天堂 777', en: 'Paradise 777', img: 'docs/images/ETAB/Parad777.png', descZh: '模拟老虎机游戏', descEn: 'Slot machine simulation game' },
    { cat: 'ETAB', zh: '长空斗士', en: 'Sky Fighter', img: 'docs/images/ETAB/SkyFight.png', descZh: '飞行棋类桌面游戏', descEn: 'Aeroplane chess board game' }
  ];

  var CATEGORIES = [
    { id: 'all', zh: '全部', en: 'All' },
    { id: 'EACT', zh: '动作', en: 'Action' },
    { id: 'EELA', zh: '教育', en: 'Educational' },
    { id: 'EPOP', zh: '精选', en: 'Featured' },
    { id: 'EPUZ', zh: '益智', en: 'Puzzle' },
    { id: 'ESPG', zh: '运动', en: 'Sport' },
    { id: 'ETAB', zh: '棋盘', en: 'Board' }
  ];

  // ================================================================
  // i18n — Translations
  // ================================================================
  var translations = {
    zh: {
      // meta
      'meta-title': 'Native32 Emulator — 让尘封的芯片游戏重获新生',
      'meta-desc': '用 Rust 编写的 Native32 游戏模拟器，支持 84 款经典芯片游戏，兼容 RetroArch',
      // nav
      'nav-features': '核心特性',
      'nav-games': '游戏库',
      'nav-arch': '技术架构',
      'nav-quickstart': '快速开始',
      // hero
      'hero-subtitle': '让尘封的芯片游戏重获新生',
      'hero-desc': '用 Rust 编写的 Native32 游戏模拟器，完整支持 Sunplus DVD 芯片上的 84 款经典游戏',
      'hero-download': '下载',
      'hero-github': '查看源码',
      'hero-scroll': '向下滚动探索',
      // about
      'about-title': '什么是 Native32？',
      'about-p1': 'Native32 是 <strong>Sunplus</strong> 公司为 DVD 播放器和电视芯片（约 2005–2011 年）开发的游戏格式。游戏运行在芯片内置的栈式虚拟机上，使用类 ActionScript 的字节码，配合光栅图形和 MP3/MPEG-1 多媒体。',
      'about-p2': '这些曾经只能在特定硬件上运行的游戏，如今通过 Native32Emu 在现代设备上重获新生。',
      'about-chip': 'Sunplus 芯片',
      'about-chip-sub': 'DVD / TV SoC',
      'about-game': '.smf / .ssl',
      'about-game-sub': 'Native32 游戏',
      'about-emu-sub': 'Rust 模拟器',
      // stats
      'stat-games': '支持游戏',
      'stat-games-sub': '100% 兼容',
      'stat-opcodes': 'VM 操作码',
      'stat-opcodes-sub': '类 ActionScript 虚拟机',
      'stat-platforms': '目标平台',
      'stat-platforms-sub': 'Windows / macOS / Linux / Android',
      'stat-lines': 'Rust 代码行数',
      'stat-lines-sub': '零 C 依赖',
      // features
      'feat-title': '核心特性',
      'feat-subtitle': '从字节码虚拟机到 MPEG-1 解码器，全栈 Rust 实现',
      'feat-vm-title': 'Action 字节码虚拟机',
      'feat-vm-desc': '36 个操作码的栈式虚拟机，覆盖算术、逻辑、字符串、控制流、精灵和 I/O，完整还原 Native32 游戏脚本引擎。',
      'feat-mpeg-title': 'MPEG-1 解码器',
      'feat-mpeg-desc': '纯 Rust 移植 PL_MPEG，零 C 依赖的 MPEG-1 视频 + MP2 音频解码器，流畅播放过场动画。',
      'feat-dual-title': '双前端架构',
      'feat-dual-desc': '平台无关的核心引擎 + 独立的 Standalone 和 RetroArch 前端，共享 100% 模拟逻辑。',
      'feat-zip-title': 'ZIP 包加载',
      'feat-zip-desc': '直接加载 .zip 游戏包，自动解压并启动 FHUI 或 NA32UI 主菜单，还原原始的游戏浏览体验。',
      'feat-des-title': '金手指系统',
      'feat-des-desc': '运行时修改 VM 变量、精灵属性和帧状态，支持调试日志发现可用目标，轻松实现无限生命、跳关等作弊效果。',
      'feat-retro-title': 'RetroArch 核心',
      'feat-retro-desc': '完整的 libretro 核心，支持 RetroPad 映射、核心选项、着色器、网络对战等 RetroArch 生态功能。',
      // gallery
      'gallery-title': '游戏库',
      'gallery-subtitle': '84 款游戏，7 大分类，全部通过测试',
      // architecture
      'arch-title': '技术架构',
      'arch-subtitle': '清晰的三层架构，平台无关的核心引擎',
      'arch-frontends': '前端',
      'arch-standalone': 'Standalone 可执行文件',
      'arch-standalone-sub': 'minifb 窗口',
      'arch-libretro': 'libretro cdylib',
      'arch-libretro-sub': 'RetroArch 核心',
      'arch-core': '核心引擎',
      'arch-core-sub': '平台无关的库',
      'arch-platforms': '目标平台',
      // quickstart
      'qs-title': '快速开始',
      'qs-subtitle': '几行命令，即刻体验',
      'qs-standalone': 'Standalone',
      'qs-standalone-1': '下载最新版本',
      'qs-standalone-1-sub': '从 Releases 页面下载对应平台的二进制文件',
      'qs-standalone-2': '运行游戏',
      'qs-standalone-3': '或从 ZIP 加载',
      'qs-retro': 'RetroArch',
      'qs-retro-1': '下载 libretro 核心',
      'qs-retro-1-sub': '从 Releases 页面下载对应平台的核心文件',
      'qs-retro-2': '安装核心',
      'qs-retro-2-sub': '复制到 RetroArch 的 cores/ 目录',
      'qs-retro-3': '加载核心并启动',
      'qs-build': '从源码编译',
      'qs-build-1': '克隆仓库',
      'qs-build-2': '编译 Standalone',
      'qs-build-3': '或编译 RetroArch 核心',
      // footer
      'footer-desc': '用 Rust 编写的 Native32 游戏模拟器',
      'footer-project': '项目',
      'footer-contributing': '贡献指南',
      'footer-community': '社区',
      'footer-docs': '文档',
      'footer-cli': '独立模拟器',
      'footer-core': 'RetroArch Core',
      'footer-gamelist': '游戏列表',
      'footer-copy': 'BSD 3-Clause License &copy; 2025 Aloys. Built with 🦀 Rust.'
    },
    en: {
      // meta
      'meta-title': 'Native32 Emulator — Bring forgotten chip games back to life',
      'meta-desc': 'A Native32 game emulator written in Rust, supporting 84 classic chip games with RetroArch compatibility',
      // nav
      'nav-features': 'Features',
      'nav-games': 'Games',
      'nav-arch': 'Architecture',
      'nav-quickstart': 'Quick Start',
      // hero
      'hero-subtitle': 'Bring forgotten chip games back to life',
      'hero-desc': 'A Native32 game emulator written in Rust, fully supporting 84 classic games from Sunplus DVD chips',
      'hero-download': 'Download',
      'hero-github': 'View Source',
      'hero-scroll': 'Scroll to explore',
      // about
      'about-title': 'What is Native32?',
      'about-p1': 'Native32 is a game format developed by <strong>Sunplus</strong> for DVD player and TV chipsets (circa 2005–2011). Games run on a stack-based virtual machine built into the chip, using ActionScript-like bytecode with raster graphics and MP3/MPEG-1 multimedia.',
      'about-p2': 'Games that once could only run on specific hardware now come back to life on modern devices through Native32Emu.',
      'about-chip': 'Sunplus Chip',
      'about-chip-sub': 'DVD / TV SoC',
      'about-game': '.smf / .ssl',
      'about-game-sub': 'Native32 Games',
      'about-emu-sub': 'Rust Emulator',
      // stats
      'stat-games': 'Games Supported',
      'stat-games-sub': '100% compatibility',
      'stat-opcodes': 'VM Opcodes',
      'stat-opcodes-sub': 'ActionScript-like VM',
      'stat-platforms': 'Platforms',
      'stat-platforms-sub': 'Windows / macOS / Linux / Android',
      'stat-lines': 'Lines of Rust',
      'stat-lines-sub': 'Zero C dependencies',
      // features
      'feat-title': 'Core Features',
      'feat-subtitle': 'From bytecode VM to MPEG-1 decoder — full-stack Rust implementation',
      'feat-vm-title': 'Action Bytecode VM',
      'feat-vm-desc': 'A 36-opcode stack-based virtual machine covering arithmetic, logic, strings, control flow, sprites, and I/O — fully recreating the Native32 game scripting engine.',
      'feat-mpeg-title': 'MPEG-1 Decoder',
      'feat-mpeg-desc': 'Pure-Rust port of PL_MPEG — a zero-C-dependency MPEG-1 video + MP2 audio decoder for smooth cutscene playback.',
      'feat-dual-title': 'Dual Frontend',
      'feat-dual-desc': 'A platform-independent core engine with separate Standalone and RetroArch frontends, sharing 100% of the emulation logic.',
      'feat-zip-title': 'ZIP Archive Loading',
      'feat-zip-desc': 'Load .zip game packages directly — auto-extract and launch the FHUI or NA32UI main menu for the original browsing experience.',
      'feat-des-title': 'Cheat System',
      'feat-des-desc': 'Modify VM variables, sprite properties, and frame state at runtime. Debug logging helps discover cheat targets for infinite lives, level skips, and more.',
      'feat-retro-title': 'RetroArch Core',
      'feat-retro-desc': 'A complete libretro core with RetroPad mapping, core options, shaders, netplay, and the full RetroArch ecosystem.',
      // gallery
      'gallery-title': 'Game Gallery',
      'gallery-subtitle': '84 games, 7 categories, all passing tests',
      // architecture
      'arch-title': 'Architecture',
      'arch-subtitle': 'Clean three-layer architecture with a platform-independent core engine',
      'arch-frontends': 'Frontends',
      'arch-standalone': 'native32emu',
      'arch-standalone-sub': 'Standalone binary · minifb window',
      'arch-libretro': 'native32emu-libretro',
      'arch-libretro-sub': 'libretro cdylib · RetroArch core',
      'arch-core': 'Core Engine',
      'arch-core-sub': 'Platform-independent library',
      'arch-platforms': 'Platforms',
      // quickstart
      'qs-title': 'Quick Start',
      'qs-subtitle': 'A few commands to get started',
      'qs-standalone': 'Standalone',
      'qs-standalone-1': 'Download latest release',
      'qs-standalone-1-sub': 'Get the binary for your platform from the Releases page',
      'qs-standalone-2': 'Run a game',
      'qs-standalone-3': 'Or load from ZIP',
      'qs-retro': 'RetroArch',
      'qs-retro-1': 'Download libretro core',
      'qs-retro-1-sub': 'Get the core for your platform from the Releases page',
      'qs-retro-2': 'Install the core',
      'qs-retro-2-sub': 'Copy to RetroArch\'s cores/ directory',
      'qs-retro-3': 'Load core and start',
      'qs-build': 'Build from Source',
      'qs-build-1': 'Clone the repository',
      'qs-build-2': 'Build Standalone',
      'qs-build-3': 'Or build RetroArch core',
      // footer
      'footer-desc': 'A Native32 game emulator written in Rust',
      'footer-project': 'Project',
      'footer-contributing': 'Contributing',
      'footer-community': 'Community',
      'footer-docs': 'Docs',
      'footer-cli': 'Standalone Emulator',
      'footer-core': 'RetroArch Core',
      'footer-gamelist': 'Game List',
      'footer-copy': 'BSD 3-Clause License &copy; 2025 Aloys. Built with 🦀 Rust.'
    }
  };

  var currentLang = localStorage.getItem('n32-lang') || (navigator.language.startsWith('zh') ? 'zh' : 'en');

  // ================================================================
  // i18n — Apply translations
  // ================================================================
  function applyLang(lang) {
    currentLang = lang;
    localStorage.setItem('n32-lang', lang);
    document.documentElement.lang = lang === 'zh' ? 'zh-CN' : 'en';

    var t = translations[lang];

    // Update text content for elements with data-i18n
    document.querySelectorAll('[data-i18n]').forEach(function (el) {
      var key = el.getAttribute('data-i18n');
      if (t[key] === undefined) return;
      // Skip title/meta — handled separately below
      if (el.tagName === 'TITLE' || el.tagName === 'META') return;
      el.innerHTML = t[key];
    });

    // Update <title> and meta description
    if (t['meta-title']) document.title = t['meta-title'];
    var metaDesc = document.querySelector('meta[name="description"]');
    if (metaDesc && t['meta-desc']) metaDesc.setAttribute('content', t['meta-desc']);

    // Update language toggle button text
    var langBtn = document.getElementById('lang-toggle');
    if (langBtn) langBtn.textContent = lang === 'zh' ? 'EN' : '中';

    // Rebuild gallery with correct language
    buildGallery();
  }

  // ================================================================
  // GALLERY — Tab + Carousel
  // ================================================================
  var currentTab = 'all';

  function getCatCount(catId) {
    if (catId === 'all') return GAMES.length;
    return GAMES.filter(function (g) { return g.cat === catId; }).length;
  }

  function buildGallery() {
    var container = document.getElementById('gallery-dynamic');
    if (!container) return;

    var lang = currentLang;
    var html = '';

    // Tab bar
    html += '<div class="gallery-tabs">';
    CATEGORIES.forEach(function (cat) {
      var count = getCatCount(cat.id);
      var label = lang === 'zh' ? cat.zh : cat.en;
      var active = cat.id === currentTab ? ' active' : '';
      html += '<button class="gallery-tab' + active + '" data-cat="' + cat.id + '">' + label + ' (' + count + ')</button>';
    });
    html += '</div>';

    // Carousel for each category (only show active tab)
    CATEGORIES.forEach(function (cat) {
      if (cat.id !== currentTab) return;
      var games = cat.id === 'all' ? GAMES : GAMES.filter(function (g) { return g.cat === cat.id; });
      var catLabel = lang === 'zh' ? cat.zh : cat.en;

      html += '<div class="carousel-wrapper">';
      html += '<button class="carousel-btn carousel-prev" aria-label="Previous">&#8249;</button>';
      html += '<div class="carousel-viewport">';
      html += '<div class="carousel-track" data-cat="' + cat.id + '">';

      games.forEach(function (g, i) {
        var name = lang === 'zh' ? g.zh + ' ' + g.en : g.en;
        var desc = lang === 'zh' ? g.descZh : g.descEn;
        html += '<div class="carousel-card">';
        html += '  <img src="' + g.img + '" alt="' + g.en + '" loading="lazy">';
        html += '  <div class="carousel-card-overlay">';
        html += '    <span class="gallery-tag">' + catLabel + '</span>';
        html += '    <h4>' + name + '</h4>';
        html += '    <p>' + desc + '</p>';
        html += '  </div>';
        html += '</div>';
      });

      html += '</div>';
      html += '</div>';
      html += '<button class="carousel-btn carousel-next" aria-label="Next">&#8250;</button>';

      // Dots
      var cardsPerView = window.innerWidth > 768 ? 4 : (window.innerWidth > 480 ? 2 : 1);
      var totalPages = Math.ceil(games.length / cardsPerView);
      html += '<div class="carousel-dots">';
      for (var d = 0; d < totalPages; d++) {
        html += '<span class="carousel-dot' + (d === 0 ? ' active' : '') + '" data-page="' + d + '"></span>';
      }
      html += '</div>';

      html += '</div>';
    });

    container.innerHTML = html;

    // Bind tab clicks
    container.querySelectorAll('.gallery-tab').forEach(function (tab) {
      tab.addEventListener('click', function () {
        currentTab = tab.getAttribute('data-cat');
        buildGallery();
      });
    });

    // Bind carousel controls
    initCarousel();
  }

  function initCarousel() {
    document.querySelectorAll('.carousel-wrapper').forEach(function (wrapper) {
      var viewport = wrapper.querySelector('.carousel-viewport');
      var track = wrapper.querySelector('.carousel-track');
      var prevBtn = wrapper.querySelector('.carousel-prev');
      var nextBtn = wrapper.querySelector('.carousel-next');
      var dots = wrapper.querySelectorAll('.carousel-dot');
      if (!viewport || !track) return;

      var page = 0;

      function getCardsPerView() {
        return window.innerWidth > 768 ? 4 : (window.innerWidth > 480 ? 2 : 1);
      }

      function getTotalPages() {
        var cards = track.querySelectorAll('.carousel-card');
        return Math.ceil(cards.length / getCardsPerView());
      }

      function goTo(p) {
        var total = getTotalPages();
        page = Math.max(0, Math.min(p, total - 1));
        var cpv = getCardsPerView();
        var card = track.querySelector('.carousel-card');
        var gap = parseFloat(window.getComputedStyle(track).columnGap) || 0;
        var pageWidth = card ? cpv * (card.offsetWidth + gap) : viewport.offsetWidth;
        var maxOffset = Math.max(0, track.scrollWidth - viewport.clientWidth);
        var offset = Math.min(page * pageWidth, maxOffset);
        track.style.transform = 'translateX(-' + offset + 'px)';

        dots.forEach(function (d, i) {
          d.classList.toggle('active', i === page);
        });
      }

      if (prevBtn) prevBtn.addEventListener('click', function () { goTo(page - 1); });
      if (nextBtn) nextBtn.addEventListener('click', function () { goTo(page + 1); });

      dots.forEach(function (dot) {
        dot.addEventListener('click', function () {
          goTo(parseInt(dot.getAttribute('data-page'), 10));
        });
      });

      // Touch/swipe support
      var startX = 0;
      var isDragging = false;
      viewport.addEventListener('touchstart', function (e) {
        startX = e.touches[0].clientX;
        isDragging = true;
      }, { passive: true });
      viewport.addEventListener('touchend', function (e) {
        if (!isDragging) return;
        isDragging = false;
        var diff = startX - e.changedTouches[0].clientX;
        if (Math.abs(diff) > 50) {
          goTo(page + (diff > 0 ? 1 : -1));
        }
      }, { passive: true });
    });
  }

  // ================================================================
  // NAVBAR — Scroll effect
  // ================================================================
  var navbar = document.getElementById('navbar');

  function onScroll() {
    navbar.classList.toggle('scrolled', window.scrollY > 50);
  }

  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // ---- Mobile nav toggle ----
  var toggle = document.querySelector('.nav-toggle');
  var navLinks = document.querySelector('.nav-links');

  if (toggle && navLinks) {
    toggle.addEventListener('click', function () {
      navLinks.classList.toggle('open');
    });
    navLinks.querySelectorAll('a').forEach(function (a) {
      a.addEventListener('click', function () { navLinks.classList.remove('open'); });
    });
  }

  // ================================================================
  // SCROLL REVEAL — Intersection Observer
  // ================================================================
  var fadeEls = document.querySelectorAll('.fade-in-up');

  if ('IntersectionObserver' in window) {
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add('visible');
          observer.unobserve(entry.target);
        }
      });
    }, { threshold: 0.1, rootMargin: '0px 0px -40px 0px' });

    fadeEls.forEach(function (el) { observer.observe(el); });
  } else {
    fadeEls.forEach(function (el) { el.classList.add('visible'); });
  }

  // ================================================================
  // ANIMATED COUNTER
  // ================================================================
  var statNumbers = document.querySelectorAll('.stat-number[data-target]');

  function animateCounter(el) {
    var target = parseInt(el.dataset.target, 10);
    var suffix = el.dataset.suffix || '';
    var duration = 1800;
    var start = performance.now();

    function tick(now) {
      var elapsed = now - start;
      var progress = Math.min(elapsed / duration, 1);
      var eased = 1 - Math.pow(1 - progress, 3);
      var current = Math.round(eased * target);
      el.textContent = current.toLocaleString() + suffix;
      if (progress < 1) requestAnimationFrame(tick);
    }

    requestAnimationFrame(tick);
  }

  if ('IntersectionObserver' in window) {
    var statObserver = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          animateCounter(entry.target);
          statObserver.unobserve(entry.target);
        }
      });
    }, { threshold: 0.5 });

    statNumbers.forEach(function (el) { statObserver.observe(el); });
  } else {
    statNumbers.forEach(function (el) { animateCounter(el); });
  }

  // ================================================================
  // CIRCUIT CANVAS — Hero background
  // ================================================================
  var canvas = document.getElementById('circuit-canvas');
  if (canvas && canvas.getContext) {
    var ctx = canvas.getContext('2d');
    var w, h, particles;
    var PARTICLE_COUNT = 60;
    var LINE_DIST = 120;

    function resize() {
      w = canvas.width = canvas.offsetWidth;
      h = canvas.height = canvas.offsetHeight;
    }

    function initParticles() {
      particles = [];
      for (var i = 0; i < PARTICLE_COUNT; i++) {
        particles.push({
          x: Math.random() * w,
          y: Math.random() * h,
          vx: (Math.random() - 0.5) * 0.4,
          vy: (Math.random() - 0.5) * 0.4,
          r: Math.random() * 2 + 1
        });
      }
    }

    function draw() {
      ctx.clearRect(0, 0, w, h);

      for (var i = 0; i < particles.length; i++) {
        for (var j = i + 1; j < particles.length; j++) {
          var dx = particles[i].x - particles[j].x;
          var dy = particles[i].y - particles[j].y;
          var dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < LINE_DIST) {
            var alpha = (1 - dist / LINE_DIST) * 0.4;
            ctx.strokeStyle = 'rgba(74, 222, 128, ' + alpha + ')';
            ctx.lineWidth = 0.5;
            ctx.beginPath();
            ctx.moveTo(particles[i].x, particles[i].y);
            ctx.lineTo(particles[j].x, particles[j].y);
            ctx.stroke();
          }
        }
      }

      particles.forEach(function (p) {
        ctx.fillStyle = 'rgba(232, 85, 58, 0.6)';
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        ctx.fill();

        p.x += p.vx;
        p.y += p.vy;

        if (p.x < 0) p.x = w;
        if (p.x > w) p.x = 0;
        if (p.y < 0) p.y = h;
        if (p.y > h) p.y = 0;
      });

      requestAnimationFrame(draw);
    }

    resize();
    initParticles();
    draw();

    window.addEventListener('resize', function () {
      resize();
      initParticles();
    });
  }

  // ================================================================
  // SMOOTH SCROLL
  // ================================================================
  document.querySelectorAll('a[href^="#"]').forEach(function (anchor) {
    anchor.addEventListener('click', function (e) {
      var target = document.querySelector(anchor.getAttribute('href'));
      if (target) {
        e.preventDefault();
        target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    });
  });

  // ================================================================
  // LANGUAGE TOGGLE
  // ================================================================
  var langBtn = document.getElementById('lang-toggle');
  if (langBtn) {
    langBtn.addEventListener('click', function () {
      applyLang(currentLang === 'zh' ? 'en' : 'zh');
    });
  }

  // ================================================================
  // INIT
  // ================================================================
  applyLang(currentLang);
  buildGallery();

  // Re-init scroll reveal for dynamically added elements
  if ('IntersectionObserver' in window) {
    var revealObserver = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add('visible');
          revealObserver.unobserve(entry.target);
        }
      });
    }, { threshold: 0.1, rootMargin: '0px 0px -40px 0px' });

    // Observe new elements after gallery build
    setTimeout(function () {
      document.querySelectorAll('.fade-in-up:not(.visible)').forEach(function (el) {
        revealObserver.observe(el);
      });
    }, 100);
  }

})();
