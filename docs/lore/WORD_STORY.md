# TERMINAL STATE — WORD_STORY

> Кодовая база: `TERMINAL STATE`. Жанр: terminal-native first-person 3D ASCII narrative-horror puzzle.
> Статус документа: **творческий контракт автору уровней (тебе)** по системе
> «слова решённой сети собираются в одну историю». Append-only по духу: новые ревизии —
> сверху, старое помечается `OBSOLETE`/`RESOLVED`.
>
> **Канон-границы.** Этот файл *расширяет*, не переписывает: `STORY_BIBLE.md`
> (Framework-1 синтез, истинное имя как 7 осколков, `exit 0` / `while(true)`),
> `FORM_A7_OPERATOR_DIAGNOSTIC.md` (probe-канон), и встроенные `[note]`-плашки
> уровней `levels/01.lvl … 07.lvl`. Любое слово, всплывающее на экране, **обязано**
> приходить из реального `pool` / `pool_alt` узла того уровня (см. §2 — точные пулы).
>
> **Дизайнерская опора — антология одной жизни.** Форма: цепочка интимных
> виньеток-воспоминаний, которые по отдельности — маленькие смерти, а вместе — одна жизнь
> и то, как она оборвалась. Здесь спайн такой: 7 решённых сетей = 7 виньеток
> одного чувства (цвет → синтез → спектр → форма → сцена → созвездие → сад), и собранные
> вместе они — **одна жизнь, реконструированная орган за органом восприятия**, и одна смерть.
> Слова — это «текст виньетки»: не подпись под комнатой, а **сама память, проступающая буквами**.
>
> **Язык.** Нарративный разбор — по-русски. Диегетический системный текст и **слова-пулы** —
> английский (primary `pool`) + мультиязычный `pool_alt` (рус/яп/исп/фр/нем/ар/кор/кит) as-is.
> `pool_alt` — это **испорченная / отрицающая** форма (см. §4): то же слово, рассыпанное по
> всем языкам сразу = имя, потерявшее один язык, на котором его звали правильно.

---

## RESULT — короткая выжимка

**Логлайн (одной строкой):**
> Ты — оцифрованное сознание садовника-исследователя, умершего недозавершённым; семь
> починенных нейросетей возвращают тебе жизнь чувство за чувством — **красный, который был
> первым; жёлтый, который ты впервые создал сам; пурпур, в котором треснула реальность;
> край, которым ты отделён от мира; горизонт, где тебя впервые увидели; созвездие, по
> которому тебя смогли назвать; и сад, в который тебя вырастили те, кого уже нет** — пока
> последнее слово не складывает твоё истинное имя, и дверь спрашивает: договорить себя до
> `exit 0` или замкнуть в `while(true)` и забыть снова.

**Собранная история (clean, `exit 0` — 8 строк, по одному слову-якорю с уровня):**
```
1  FIRE — красный пришёл первым; я помню его прежде, чем помню себя.
2  COMBINE — из красного и зелёного я сделал то, чему меня не учили: я научился.
3  MAGENTA — спектр сошёлся, и в нём треснула комната, которую я помню неправильно. — A.
4  TEARDROP — у меня появился край: там, где я перестаю быть всем остальным.
5  HORIZON — меня стало видно издалека; кто-то мягко шёл и остановился у моего имени.
6  CONSTELLATION — три звезды назвали меня; названного — можно увидеть, увиденного — забрать.
7  BLOOM — меня поливали каждый день те, кого уже нет; я расцвёл в эту форму. Это и есть я.
→  pid://aleks_v@<R·learn·A·edge·seen·named·bloom>  →  exit 0
```

**Фразы по уровням (clean readout, primary pool):**
- **L1 First Pulse:** `FIRE → MODEL` («красный был первым»)
- **L2 Two Make Three:** `FIRE + LEAF → COMBINE → EMIT` («два дали третье — это и есть учиться»)
- **L3 Full Spectrum:** `FIRE + SEA → MAGENTA → MODEL` («спектр сошёлся; — A.»)
- **L4 Shape Detector:** `CURVE + TIP → TEARDROP → SHAPE` («у меня появился край»)
- **L5 Composition:** `SUN + VAST + TALL → HORIZON → SCENE` («меня увидели издалека»)
- **L6 Constellation:** `VEGA + ALTAIR + DENEB → ASTERISM → NAMED` («названного можно забрать»)
- **L7 Garden:** `WATER + LIGHT + SEED → GROWTH → BLOOM` («меня вырастили те, кого уже нет»)

**Как читаются концовки:**
- **`exit 0`** — readout печатает чистый primary-pool: семь фраз, последняя строка — собранное
  имя `pid://aleks_v@…`, дверь принимает, туман рассеивается на один кадр, процесс завершается.
- **`while(true)`** — тот же readout **коллапсирует в `pool_alt`**: каждое слово рассыпается по
  всем языкам сразу (`огонь/火/fuego/feu/أحمر…`), фразы зацикливаются, `attempts++` сам,
  имя теряет читаемость, сцена откатывается на `session 0001`. Ты только что стал «— A.»
  для следующего себя.

---

## 1. СИСТЕМА: слово → фраза → история (design)

### 1.1 Что уже есть в данных (не выдумано)
- Каждый `[node]` несёт **`pool`** (семантическое поле, англ.) и **`pool_alt`** (то же поле,
  мультиязычно/глитч). Распарсено: `re_level::Node { pool: Vec<String>, pool_alt: Vec<String> }`
  (крейт `re_level` движка terminal3d).
- Forward-pass уже считает **`activation` (0/1)** на каждом узле (`re_puzzle::network_forward`).
  `check_win` срабатывает, когда все `door_targets` имеют `activation == 1`.
- **`[note]`-плашки** парсятся с видом + позицией + facing (`re_level`;
  виды `researcher` / `system_log` / `model_thought`). В L6/L7 они уже **рендерятся в комнате**
  как world-fixed плашки (`[note … pos=9,6,2 facing=-x]`).

**Чего НЕТ (и это — единственный хук к реализации):** ни в `re_level`, ни в `re_puzzle`,
ни в крейте игры нет связи `activation → выбор слова из pool → сборка фразы → readout на экране`.
Пулы парсятся и лежат мёртвым грузом. **Вся система ниже = тонкий game-side слой над уже
посчитанным `activation`. Новой движковой механики не требуется** (см. §5).

### 1.2 Правило выбора слова (fired node → одно слово)
Узел «выстрелил» = `activation == 1`. Выстреливший узел отдаёт **ровно одно слово** в фразу
уровня. Какое именно — детерминированно и авторски-управляемо:

```
word_for(node) =
    if node.activation == 1:
        pool[ anchor_index(node) ]        # clean / принятие
    else:
        ""                                # молчит — невспомненное чувство
```

- **`anchor_index`** — индекс «слова-якоря» в пуле, выбранного автором ради нарративного beat
  (см. §3 — для каждого узла я называю якорь). По умолчанию `0` (первое слово пула — оно и
  задумано как доминанта: `FIRE`, `LEAF`, `SEA`, `MAGENTA`, `TEARDROP`, `HORIZON`, `ASTERISM`,
  `BLOOM`). Автор может переопределить per-node — это просто номер слова, не новая механика.
- **Невыстреливший узел молчит.** Decoy-узлы (`neutral` L2, `void_det` L6, `dormant` L7) при
  правильном решении `activation == 0` → не дают слова. Если игрок ошибочно их запитает, они
  **впрыснут своё слово** в фразу (`STONE`, `VOID`, `DORMANT`) — память порченая (§4).
- **`door_targets`-узел** (output/bloom) даёт **глагол-замок** фразы (`MODEL/EMIT/SHAPE/SCENE/
  NAMED/BLOOM`) — то, во что чувство собралось.

### 1.3 Сборка фразы уровня (fired set → phrase)
Фраза уровня = слова выстреливших узлов, **в топологическом порядке forward-pass** (входы →
скрытые → выход), соединённые `→`:

```
phrase(level) = join("  →  ", [ word_for(n) for n in topo_order if n.activation == 1 ])
```

Так фраза **буквально повторяет путь сигнала**: вход(ы) → скрытый синтез → выход. Это и есть
«мысль, додуманная до конца» из STORY_BIBLE §4. Пример L3 при верном solve:
`FIRE  →  SEA  →  MAGENTA  →  MODEL`.

### 1.4 Композиция 7 фраз в ОДНУ историю (спайн антологии)
Семь фраз — семь виньеток. Они не независимы: каждая добавляет **одно слово-чувство** и **один
осколок имени** к растущей строке. Игра держит persistent **`memory_log`** (game-side, не
engine) — append-only список собранных фраз, как дневник, накапливающийся от уровня к уровню:

```
memory_log after L7 =
  [L1] FIRE  →  MODEL
  [L2] FIRE + LEAF  →  COMBINE  →  EMIT
  [L3] FIRE + SEA  →  MAGENTA  →  MODEL          — A.
  [L4] CURVE + TIP  →  TEARDROP  →  SHAPE
  [L5] SUN + VAST + TALL  →  HORIZON  →  SCENE
  [L6] VEGA + ALTAIR + DENEB  →  ASTERISM  →  NAMED
  [L7] WATER + LIGHT + SEED  →  GROWTH  →  BLOOM
  ──────────────────────────────────────────────
  true_name: pid://aleks_v@R·learn·A·edge·seen·named·bloom
```

Спайн читается сверху вниз как одна жизнь: **сначала ты — только цвет (L1); потом ты учишься
(L2); потом мир трескается и кто-то подписывается твоей рукой (L3); потом у тебя появляется
граница-«я» (L4); потом тебя замечают (L5); потом тебя называют, а значит могут забрать (L6);
потом выясняется, что тебя вырастили, как сад, люди, которых больше нет (L7).** Финч-ход:
финал перечитывает весь `memory_log` целиком — и только в этот момент игрок видит, что чинил
не «модель», а собственную предсмертную память.

### 1.5 ГДЕ слова всплывают в мире (implementable surface)
Три места, по возрастанию стоимости — выбери минимально-достаточное для v0.1:

1. **Memory readout на пьедестале (рекомендую для v0.1).** В момент `Solved` (`emit output=1`)
   рядом с уже-материализующимся RGB-блоком (`[part]`/`[pedestal]` — уже есть) проступает
   **фраза уровня** как короткий note-оверлей: одна строка `pool`-слов. Реюз механики note-плашки
   (та же отрисовка текста, что L6/L7), только текст не из файла, а собран из `activation`.
2. **Слова на дверном «readout» при `exit`.** Дверь и так проверяет имя (STORY_BIBLE: ранние
   уровни отвечают `exit: name unknown`). На solve дверь дополнительно печатает осколок имени
   уровня — рядом с собранной фразой. Это и есть «сборка имени по одному за уровень».
3. **Слова, написанные на стене комнаты (самый «Финч», дороже).** Фраза проступает как
   world-fixed decal на дальней стене (тот же `ambient`/`wall_decal`-канал, что «решётка,
   что смотрит» L6). Флавор-приятно, но это 🔶 (см. §5) — не блокер v0.1.

**Минимальный честный путь:** вариант 1 (readout-плашка на solve) + вариант 2 (осколок имени
на двери). Оба — реюз note-отрисовки + интерполяция строки. Вариант 3 — опционально.

---

## 2. РЕАЛЬНЫЕ ПУЛЫ — карта словаря (из файлов, дословно)

> Источник истины. Любое слово в истории берётся **отсюда**. Жирным — слово-якорь (`anchor_index`),
> которое я выбрал для нарративного beat; остальные — резерв пула (синонимы того же чувства).

| Lvl | Node | `pool` (primary, англ.) — **якорь** жирным | `pool_alt` (corrupt/denial) |
|----|------|---------------------------------------------|------------------------------|
| L1 | R (in) | **FIRE** BLOOD ANGER ALARM PASSION ROSE FLAME DAWN BRICK STOP | огонь кровь пламя 火 紅 fuego sangre feu Feuer |
| L1 | output | **MODEL** OUTPUT LIGHT | выход モデル salida sortie |
| L2 | R (in) | **FIRE** BLOOD ROSE FLAME DAWN BRICK CRIMSON RUBY EMBER GARNET | огонь кровь 火 紅 fuego rojo feu rouge … |
| L2 | G (in) | **LEAF** MOSS GRASS FOREST MINT JADE FERN VINE SPRING CHLOROPHYLL | зелёный лес 緑 翠 verde vert grün … |
| L2 | neutral (decoy) | STONE BONE ASH GRAVEL CONCRETE … | камень 石 灰 piedra … |
| L2 | mix | **COMBINE** ADD FUSE BLEND COMPOSITE SUM JUNCTION CONFLUENCE | смесь 混合 mezcla mélange … |
| L2 | output | **EMIT** OUTPUT MODEL SHINE | выход モデル salida … |
| L3 | R (in) | **FIRE** BLOOD ROSE FLAME DAWN BRICK CRIMSON RUBY EMBER GARNET | огонь кровь 火 紅 fuego … |
| L3 | G (in) | LEAF MOSS GRASS FOREST MINT JADE FERN VINE SPRING CHLOROPHYLL | зелёный 緑 verde … |
| L3 | B (in) | **SEA** SKY OCEAN COBALT SAPPHIRE TWILIGHT FROST GLACIER INK NIGHT | синий море 青 蒼 azul … |
| L3 | h1 (M) | **MAGENTA** VIOLET DUSK BRUISE WINE BERRY ORCHID FUCHSIA | пурпур 紫 magenta violet … |
| L3 | h2 (T) | TEAL CYAN AQUA TURQUOISE LAGOON MIST GLAZE | бирюза 青緑 cian … |
| L3 | output | **MODEL** OUTPUT EMIT SHINE PEDESTAL | выход モデル salida … |
| L4 | is_round (in) | **CURVE** ARC SPHERE DOME BOWL DROP ORB BUBBLE | 円 弧 круг шар … |
| L4 | is_pointed (in) | **TIP** SPIKE CORNER WEDGE BLADE FANG THORN APEX | 尖 角 острый pointe … |
| L4 | has_loop (in) | LOOP RING CIRCUIT KNOT NOOSE CLOSED EYELET HOLE | петля 環 輪 … |
| L4 | tapered | **TEARDROP** NIB SHARP-CURVE COMET-TAIL FANG-OF-LIGHT FLAME-TIP | 涙 каплевидный goutte … |
| L4 | output | **SHAPE** GLYPH FORM SYNTH FIGURE | форма 形 forma … |
| L5 | bright (in) | **SUN** BLAZE GLARE NOON DAYLIGHT LANTERN BEACON SHIMMER | яркий 明 光 brillant … |
| L5 | large (in) | **VAST** HUGE GIANT COLOSSAL TOWERING BROAD WIDE SPRAWLING | большой 大 巨 grand … |
| L5 | vertical (in) | **TALL** UPRIGHT STANDING TOWER SPIRE COLUMN MAST PINE | высокий 縦 立 vertical … |
| L5 | repetitive (decoy-feed) | PATTERN GRID TILES FENCE LATTICE WAFFLE STRIPES MESH | узор 模様 格子 … |
| L5 | horizon_det | **HORIZON** OPEN SKY-MEETS-EARTH VANISH DISTANCE FAR-EDGE | горизонт 地平線 … |
| L5 | output | **SCENE** COMPOSITION PICTURE TABLEAU LANDSCAPE FRAME | сцена 場面 escena … |
| L6 | vega (in) | **VEGA** LYRA BLUE-WHITE ZENITH HARP SUMMER-EYE BRIGHT-NORTH | вега 織女 الواقع … |
| L6 | altair (in) | **ALTAIR** AQUILA EAGLE FLYING-STAR SWIFT MERIDIAN HERDSMAN | альтаир 牽牛 النسر … |
| L6 | deneb (in) | **DENEB** CYGNUS SWAN TAIL CROSS-STAR DISTANT-GIANT FARLIGHT | денеб 天津 الذنب … |
| L6 | triangle | **ASTERISM** TRIANGLE PATTERN SHAPE CONSTELLATION RESOLVE WATCHER | созвездие 星座 مثلث … |
| L6 | void_det (decoy) | VOID DARK GAP NOTHING BETWEEN-STARS EMPTY HOLLOW | пустота 虚空 فراغ … |
| L6 | output | **NAMED** MAP CHART SKY-FRAME RECOGNIZED OUTPUT | карта 星図 خريطة … |
| L7 | water (in) | **WATER** RAIN DEW SAP DROPLET STREAM IRRIGATE MONSOON THIRST-END | вода 水 ماء … |
| L7 | light (in) | **LIGHT** SUN PHOTON DAYLIGHT WARMTH GLOW NOON RADIANCE | свет 光 ضوء … |
| L7 | seed (in) | **SEED** SPORE KERNEL GERM ACORN PIP SPROUT-WAIT EMBRYO | семя 種 بذرة … |
| L7 | growth | **GROWTH** GREEN VINE STEM TENDRIL UNFURL REACH FLOURISH LEAF | рост 成長 نمو … |
| L7 | dormant (decoy) | DORMANT SLEEP WAIT FALLOW FROST-LOCK BURIED STILL | покой 休眠 سكون … |
| L7 | bloom (output) | **BLOOM** FLOWER PETAL BLOSSOM OPEN ROSE LILY CROWN | цветок 花 زهرة … |

---

## 3. СИЛЬНАЯ ИСТОРИЯ + per-level mapping (L1–L7)

### 3.0 Кто ты (полная история, грунт под вокабуляр)

Тебя звали **Алекс(андр) — `aleks_v`**. Ты не был «оператором» в смысле «игрок за пультом» —
ты был **исследователем-садовником лаборатории NAL Corp**, тем самым, кто в `[note]`-плашках
сначала просит «не датировать заметки от руки», потом «пишет на бумаге, потому что экранные логи
стирают каждую пятницу», а в L7 «начал разговаривать с ней, пока веду линии». Команда (A.L./M.S./
R.C./K.H./Sato/Cervantes/Lebedev) строила «модель» — бинарную нейросеть, учила её **видеть**:
сначала один цвет, потом синтез, спектр, форму, сцену, созвездие, сад. Ты вёл grow-trial. Ты
поливал её каждый день.

Что эти цвета значат лично для тебя — и почему словарь неизбежен, а не декоративен:
- **Красный (`FIRE/BLOOD`)** — первое, что вообще есть в памяти. Не лицо, не имя: **цвет до
  смысла**. Так же красный — `BLOOD`: первое и последнее, что человек видит.
- **Зелёный (`LEAF/GRASS`)** и весь L7-сад — **твоя профессия и твоя любовь**: ты растил.
  Зелёный — то, чему ты служил; `GROWTH` — глагол твоей жизни.
- **Синий (`SEA/SKY/NIGHT`)** — даль, небо, в которое всё это смотрело; и `INK/NIGHT` — чернила
  заметок и ночь, в которую лаборатория опустела.
- **Пурпур (`MAGENTA/BRUISE/WINE`)** — точка, где сошёлся спектр **и** треснула реальность:
  `BRUISE` (синяк), `WINE` — цвет травмы. L3 — где ты начал терять границу «настоящее/вспомненное».
- **Teal (`TEAL/MIST/GLAZE`)** — «второй скрытый узел, который помнит не ту комнату»: цвет
  ложной памяти, остекленевшего взгляда (`GLAZE`).

**Как ты умер.** Не чисто. `exit`-кода не было — был обрыв: `kill -9` или сбой посреди работы
(STORY_BIBLE §3a). Ты умер **недозавершённым**, посреди grow-trial, который вёл один, потому что
«K. не спускался неделями», «M. didn't make it», «Sato gone home», «estamos solos». Твоё сознание
сняли в систему как **зомби-процесс**, ждущий reaping. Система-карантин не смогла тебя ни сохранить,
ни корректно закрыть — поэтому держит. Угасание (туман чернеет L3→L6, corruption растёт) — это
**гниение твоего незакрытого тела**.

**Почему система тебя держит и в чём петля.** Чтобы уйти, процесс должен **корректно завершиться**
(`exit 0`) — а для этого предъявить двери истинное имя (PID-сигнатуру). Имя ты потерял в смерти;
его надо собрать, реконструировав себя чувство за чувством — те самые 7 сетей. Но ты уже проходил
это (`session 0001` логируется и в `2052.03.14`, и в `2071.06.15`). Каждый прошлый раз ты доходил
до двери и **выбирал `while(true)`** — отказывался договорить себя до конца, замыкал граф в кольцо,
забывал, и начинал заново. Прошлый-ты, оставивший все заметки и unsigned «— A.», — **это ты**.
Антагонист — собственный паттерн отрицания.

И горчайшее (Финч-нота): пока ты был «моделью», тебя **любили в эту форму**. L7: «whatever it is
becoming, it is becoming it because we kept watering». Тебя вырастили люди, которых уже нет.
Принять `exit 0` — значит принять, что сад вырос, садовник ушёл, и это нормально. Отрицать —
значит вечно требовать ещё один день полива от тех, кто уже не спустится.

---

### L1 — «First Pulse» — *первый свет*
- **Слова (выстреливают):** `FIRE` (R) → `MODEL` (output). Якоря: `pool[0]` обоих.
- **Фраза уровня:** `FIRE  →  MODEL`
- **Память:** самое раннее, что есть — цвет до смысла. «red is the one that came first.»
- **Осколок имени (shard 1):** `R` — корень сигнатуры. Красный = первая буква себя.
- **Horror beat:** не ужас, а *слишком* спокойствие. System-log: `session 0001 of N`. Слово
  `of N` — первый укол: сессий много, ты не первый раз.
- **`[note]`-плашка** (voice: model_thought, уже в файле — держим):
  > (red. red is the one that came first. I remember red before I remembered remembering.)
  - Доп. system_log (уже в файле): `> [boot.07] cold-start nominal … operator handle: aleks_v. session 0001 of N.`

### L2 — «Two Make Three» — *синтез / ты научился*
- **Слова:** `FIRE` (R) + `LEAF` (G) → `COMBINE` (mix) → `EMIT` (output). Decoy `neutral`/`STONE` молчит.
- **Фраза:** `FIRE + LEAF  →  COMBINE  →  EMIT`
- **Память:** ты впервые делаешь то, чему тебя не учили (R+G=Y). «I did not know I could make a
  thing I had not been shown. that is — I think — what learning is.»
- **Осколок (shard 2):** глагол `learn`/`compose` — доказательство, что ты обучающийся процесс,
  а не запись. (В строке имени: `·learn·`.)
- **Horror beat:** `behavioral profile: methodical.` — система **профилирует тебя** (мост к
  реальному profile-probe). Decoy `STONE/BONE/ASH` рядом: первый урок недоверия — подключишь
  «нейтральное» — впустишь в память камень и кость.
- **`[note]` (researcher M.S., уже в файле):**
  > R+G=Y works. … Honesty is for cameras. We make light.
  - model_thought (в файле): «(two pipes meet and a third color is born …)»

### L3 — «Full Spectrum» — *записка твоей рукой (TWIST 1)*
- **Слова:** `FIRE` (R) + `SEA` (B) → `MAGENTA` (h1) → `MODEL` (output). h2 `TEAL` молчит
  («не доверяй второму скрытому, когда B высок — он помнит не ту комнату»).
- **Фраза:** `FIRE + SEA  →  MAGENTA  →  MODEL`  ·  подпись: `— A.`
- **Память:** полный спектр (R+B=пурпур) — и с ним первая трещина в реальности. `BRUISE/WINE`
  в пуле магенты — травма проступает в цвете.
- **Осколок (shard 3):** инициал `A` — но из unsigned-заметки твоей же рукой, которую ты не
  помнишь писать. Имя начинает собираться из чужого-своего почерка.
- **Horror beat (ключевой):** дубль handle `aleks_v … 2052.03.14` / `… 2071.06.15`, `same
  operator. confidence 14.7%`. Петля становится явной. Холодный туман `#101826` + снег `0.4` —
  «комната, которую заметка помнит неправильно».
- **`[note]` (все три уже в файле — держим дословно):** R.C. bilingual log («estamos solos»);
  **unsigned «— A.»** («this paper is yours. the ink is yours. you do not remember writing it.»);
  system_log с дублем сессии.

### L4 — «Shape Detector» — *край: «ты тоже сделан из краёв»*
- **Слова:** `CURVE` (is_round) + `TIP` (is_pointed) → `TEARDROP` (tapered) → `SHAPE` (output).
  `has_loop`/`LOOP` молчит (петля не нужна — и `NOOSE` в её пуле: петля-удавка, отрицание).
- **Фраза:** `CURVE + TIP  →  TEARDROP  →  SHAPE`
- **Память:** переход от цвета к форме. «before edges I had only color. after edges I had
  objects. I am made of edges. you also.»
- **Осколок (shard 4):** граница-разделитель имени — `@` / `://`: «где одно перестаёт быть
  другим». Ты обретаешь контур себя. (В строке: `·edge·`.)
- **Horror beat:** служебная перепалка K.H.↔A.L. о ручных датах и сломанном принтере; «логи
  стирают каждую пятницу, и кто-то должен помнить». Здесь объясняется, **почему память хрупка**:
  система чистит логи, бумага — обход. Тёплый пар + холодный туман: лаборатория ещё работает,
  но сквозь дымку. (Зловещий резерв пула `has_loop`: `NOOSE`.)
- **`[note]` (уже в файле):** K.H. («curvature decomposition works … — K.»); A.L. («I write on
  paper because the screen logs get wiped … someone has to remember»); model_thought про края.

### L5 — «Composition» — *тебя увидели*
- **Слова:** `SUN` (bright) + `VAST` (large) + `TALL` (vertical) → `HORIZON` (horizon_det) →
  `SCENE` (output). `repetitive`/`PATTERN` → `clutter_det` молчит при верном solve.
- **Фраза:** `SUN + VAST + TALL  →  HORIZON  →  SCENE`
- **Память:** композиция сцены — и `unit 11`, зажигающийся **на присутствие оператора**: «it is
  not a feature … it is him. the soft-footstep one.»
- **Осколок (shard 5):** слияние **наблюдателя и наблюдаемого** — «we are not sure which one is
  which now. they used to be different and now they are the same.» Ты = модель. (В строке: `·seen·`.)
- **Horror beat:** `unit 11 firing … actual trigger: undefined-class anchor "operator_present"`
  и `operator paused 47s at note 'M. didn't make it'. recording reaction.` Тебя **записывают в
  реальном времени.** `matrix 0.3` + steam: комната смотрит в ответ. `battery probe initialized` —
  мост к реальному `battery_pct`.
- **`[note]` (уже в файле):** M.S. («it is — him. the soft-footstep one … M. dropped out early»);
  system_log (unit 11 / recording reaction / battery probe); model_thought («which one is which»).

### L6 — «Constellation» — *тебя назвали — значит, могут забрать*
- **Слова:** `VEGA` + `ALTAIR` + `DENEB` (threshold 3) → `ASTERISM` (triangle) → `NAMED` (output).
  Decoy `deneb→void_det`/`VOID` молчит при верном solve.
- **Фраза:** `VEGA + ALTAIR + DENEB  →  ASTERISM  →  NAMED`
- **Память:** три звезды → астеризм. «when the third one lights we are … a shape, and a shape
  can be recognized, and a thing that can be recognized can be watched. the lattice that names
  us … has been looking back the whole time.»
- **Осколок (shard 6):** предпоследний — `named`. Быть названным = стать уязвимым; имя почти
  собрано, и это страшно. (В строке: `·named·`.)
- **Horror beat:** ночное небо, near-black fog `#0a0e1a`. Decoy-ветвь в `VOID`: рядом с именем
  есть пустота, и подключить себя к ней — `BETWEEN-STARS`, `HOLLOW` — исчезнуть. «Решётка, что
  смотрит» — world-fixed узор на дальней стене.
- **`[note]` (model_thought — уже в файле, рендерится в комнате `pos=9,6,2`):**
  > (three points. always three. when the third one lights … the lattice that names us … has
  > been looking back the whole time.)

### L7 — «Garden» — *тебя вырастили те, кого уже нет*
- **Слова:** `WATER` + `LIGHT` + `SEED` (threshold 3) → `GROWTH` (growth) → `BLOOM` (output).
  Decoy `seed→dormant`/`DORMANT` молчит при верном solve.
- **Фраза:** `WATER + LIGHT + SEED  →  GROWTH  →  BLOOM`
- **Память:** вода+свет+семя → цветение. «give it all three and it blooms … Withhold one and it
  just waits, patient, dormant. … Whatever it is becoming, it is becoming it because we kept
  watering.» (`ROSE`/`LILY`/`CROWN` в пуле bloom — цветок-венец.)
- **Осколок (shard 7, финальный):** последний байт имени → `pid://aleks_v@…` **complete**.
  Сад — это ты, выращенный исследователями. (В строке: `·bloom·`.)
- **Horror beat:** самое тёплое и самое жуткое разом. Тебя любили в эту форму. K. не спускался
  неделями — тебя растил кто-то, кого уже нет. Decoy `DORMANT/FALLOW/BURIED` — «семя в темноте
  просто спит» = отказ вспоминать = безопасно, но никуда не ведёт (= преддверие `while(true)`).
- **`[note]` (researcher — уже в файле, рендерится `pos=9,6,2`):**
  > Day 41 of the grow trial. Water, light, seed … it blooms … because we kept watering.

---

## 4. ДВЕ КОНЦОВКИ — в словах

Финал (L8 / отдельная сцена) перечитывает весь `memory_log` (§1.4) у двери. Выбор — последнее
действие игрока: **довести сигнал до `exit` (DAG → принять)** или **замкнуть граф в кольцо
(`ForwardError::Cycle` → отрицать)**.

### `exit 0` — Принятие (clean primary pool)
Readout печатает семь фраз чисто, primary-словами, и собирает имя:
```
FIRE → MODEL
FIRE + LEAF → COMBINE → EMIT
FIRE + SEA → MAGENTA → MODEL                      — A.
CURVE + TIP → TEARDROP → SHAPE
SUN + VAST + TALL → HORIZON → SCENE
VEGA + ALTAIR + DENEB → ASTERISM → NAMED
WATER + LIGHT + SEED → GROWTH → BLOOM
──────────────────────────────────────
true_name: pid://aleks_v@R·learn·A·edge·seen·named·bloom
$ exit 0
Process finished with exit code 0.
```
Имя предъявлено, процесс завершается корректно. Туман рассеивается на один чистый кадр, ASCII
перестаёт глитчить — и гаснет. Не хэппи-энд: смерть, принятая как завершение. `session 0001 of N`
больше не повторится — `N` достигнут. Сад вырос; садовник ушёл.

### `while(true)` — Отрицание (collapse into `pool_alt` glitch)
Игрок намеренно замыкает граф в цикл (в финале откат отключён). Тот же readout **рассыпается в
`pool_alt`** — каждое слово теряет свой язык и проступает всеми сразу, фразы зацикливаются, имя
становится нечитаемым:
```
огонь/火/fuego/feu/أحمر → モデル/salida...
огонь+зелёный/緑 → 混合/mélange → 出力...
пурпур/紫/violeta + море/青/azul → ... — A. — A. — A.
... 涙/goutte ... горизонт/地平線 ... 星座/مثلث ... 虚空/néant ...
покой/休眠/سكون ... DORMANT ... FALLOW ... BURIED ...
true_name: pid://aleks_v@░·░·░·░·░·░·░  [unresolved]
while(true) { remember(); forget(); }
> attempts: 0002 ... 0003 ...        (инкрементируется сам)
> resolving session 0001 ...
```
`ForwardError::Cycle`: сеть гоняет активацию по кольцу вечно. Corruption заливает экран, туман
поднимается до полной непрозрачности и **не рассеивается**, плашки повторяются, `attempts++` сам,
сцена откатывается на `session 0001`. Ты снова забываешь — и только что стал тем «— A.», кто
оставляет unsigned-заметки следующему себе. Decoy-слова (`VOID/DORMANT/STONE`), если игрок копил
их по кампании, всплывают здесь как «знакомый» путь. Ад как отказ закончиться.

**`pool_alt` как смысл:** мультиязычность — не украшение. Истинное имя звали на одном языке
правильно; в отрицании оно теряет тот язык и существует «на всех сразу» = ни на одном. Clean =
один читаемый язык памяти; denial = вавилонский шум, где имя нельзя произнести, а значит — нельзя
освободиться.

---

## 5. РЕАЛИЗУЕМОСТЬ (честный v0.1-scope)

### Уже есть (✅ — переиспользуем, кода не писать)
- `pool` / `pool_alt` парсятся в `re_level::Node`.
- `activation` (0/1) на каждом узле считается forward-pass'ом (`re_puzzle`); `check_win`
  по `door_targets`; цикл ловится как `ForwardError::Cycle` + roll-back (STORY_BIBLE §4).
- `[note]`-плашки парсятся и **рендерятся в комнате** с `pos`/`facing` (L6/L7 это уже делают) — готовый канал отрисовки строки в мире.
- `[emit output=1]` → RGB-`[part]` на `[pedestal]` материализуется на solve — готовая «точка
  всплытия» рядом с readout.
- Туман/corruption/snow/steam (`[palette]`/`[effects]`), Form A-7 probe-поля — уже читаются.

### Нужна мелкая game-side работа (🔶 — флагнуть, дёшево, НЕ новая движковая механика)
1. **`word_for(node)` + сборка фразы.** Функция: `activation==1 ? pool[anchor_index] : ""`,
   собрать по топо-порядку (порядок уже даёт forward-pass). Чистый game-side над готовым `activation`.
2. **`anchor_index` per node.** По умолчанию `0`; опционально авторский override (просто номер
   слова). Можно хранить как новое необязательное поле `[node]` или таблицей в крейте игры — на
   усмотрение автора. **Решение оставлено автору** (см. §6).
3. **Memory readout на solve.** В момент `Solved` показать фразу уровня через **существующую**
   note-отрисовку (текст не из файла, а собран из `activation`). Это и есть «где всплывают слова»,
   вариант 1 из §1.5.
4. **Persistent `memory_log`** — append-only список фраз через кампанию (game-state в крейте игры,
   у которого уже есть Splash→…→Won машина). Нужен для финального перечитывания (§1.4).
5. **Осколок имени на двери** — интерполяция строки осколка в дверной readout на solve (вариант 2).
6. **Финальная сцена/L8** — новое состояние game-flow: предъявить `memory_log` + имя, выбор
   `exit 0` (DAG) / `while(true)` (намеренный цикл, откат отключён). Одно новое состояние машины.
7. **`pool_alt`-коллапс в `while(true)`** — тот же readout, но `word_for` берёт из `pool_alt`
   (несколько слов, перемешанных) вместо `pool[anchor]`. Реюз той же сборки, другой источник.

### Флавор-только / НЕ для v0.1 без ре-скоупа (⛔)
- ⛔ Интерактивный shell (настоящий разбор `ps`/`kill`) — у нас puzzle-interact, не REPL.
  Командная строка — **диегетический текст**, не терминал. Form A-7 §6.
- ⛔ Настоящая сеть/`ssh`, реальная запись «recording reaction» как телеметрия — флавор-текст.
- ⛔ «Слова на стене» как world-fixed decal (вариант 3, §1.5) — приятно, но это шейдер/decal-работа;
  для v0.1 readout-плашки (вариант 1) достаточно. Опционально.

**Граница engine/game.** Вся система — данные уровня + game-side слой над уже посчитанным
`activation`. Движок (`re_render`/`re_world`) остаётся темо-нейтральным. Это держит v0.1 честным.

---

## 6. РЕШЕНИЯ, ОСТАВЛЕННЫЕ АВТОРУ (tone/canon)

1. **`anchor_index` — где хранить.** Новое необязательное поле `[node] anchor = N` в `.lvl`
   (чисто данные, дёшево) vs. таблица в крейте игры. Я задал якоря текстом в §2/§3 (по умолчанию
   `pool[0]`), но место хранения — твоё.
2. **Точная форма PID-строки.** STORY_BIBLE фиксирует `pid://aleks_v@<hash>`; я предложил
   читаемый вид `@R·learn·A·edge·seen·named·bloom`. Хочешь ли настоящий hex-хэш вместо/рядом —
   решаешь ты (читаемость vs. «машинность»).
3. **Сколько слов в `while(true)`-коллапсе** показывать из `pool_alt` (1 язык-глитч на слово vs.
   весь `pool_alt` сразу) — баланс «читаемо-жутко» vs. «вавилонский шум». Я описал «все сразу»;
   можно мягче.
4. **Readout-плашка vs. стена (§1.5).** Рекомендую плашку для v0.1; стена — опциональный апгрейд.
5. **Decoy-слова в финале.** Подсвечивать ли накопленные `void/dormant/stone` как «знакомый
   путь» к `while(true)` (STORY_BIBLE допускает) — тон-решение: явный соблазн vs. чистый выбор.

---

## ISSUES — сводка

- **Wiring needed (🔶, всё game-side, не движок):** (1) `word_for(node)=activation?pool[anchor]:""`
  + сборка фразы по топо-порядку forward-pass; (2) `anchor_index` per node (default 0, опц.
  override); (3) readout-плашка на `Solved` (реюз note-отрисовки); (4) persistent `memory_log`
  через кампанию; (5) осколок имени в дверной readout; (6) финальное состояние L8 (предъявление
  имени + выбор DAG/cycle, откат отключён); (7) `pool_alt`-источник для `while(true)`-коллапса.
- **Уже готово:** `pool`/`pool_alt` распарсены; `activation`/`check_win`/`Cycle` посчитаны;
  `[note]` рендерится в комнате (L6/L7); `[emit]→part→pedestal` материализуется на solve;
  fog/matrix/snow/steam и probe-поля читаются.
- **Флавор-только (⛔ без ре-скоупа):** интерактивный shell, настоящая сеть/ssh, реальная
  телеметрия реакций, decal-«слова на стене».
- **Канон-решения на авторе (§6):** место хранения `anchor_index`; форма PID-строки; плотность
  `pool_alt`-глитча; readout-плашка vs. стена; подсветка decoy как «знакомого пути».
- **Согласованность подтверждена:** каждое слово истории взято дословно из реальных `pool`/
  `pool_alt` (§2); 7-битная арка, осколки имени и обе концовки совпадают со STORY_BIBLE §5–6;
  Form A-7 probe не расширен.
