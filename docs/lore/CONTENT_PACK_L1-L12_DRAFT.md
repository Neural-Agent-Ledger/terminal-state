# TERMINAL STATE — CONTENT PACK L1–L12 (фаза-2 черновик, ready-to-drop)

> Кодовая база: `TERMINAL STATE`. Дата: 2026-06-17.
> Статус: **черновик контента фазы 2.** Реализует нарративный дизайн v3.
>
> **Этот файл НЕ редактирует существующие `.lvl` / `lore`.** Он — *исходник строк* для вставки
> в `.lvl` при реальной сборке уровней. Существующие плашки L1–L7 держим дословно; ниже для
> L1–L7 даны **только добавления** (атмо-точка + neuron-voice + подтверждение readout), а не
> переписывание. Для L8–L12 (которых ещё нет) — полный набор: пулы, readout, 3 плашки, атмо-точка,
> neuron-voice.
>
> **Правила голоса:**
> - Любая диегетическая строка — **по-английски** командной строки. `pool_alt` — мультиязычный.
> - 4 регистра: `researcher` (буднично, аффект смещён), `system_log` (`>`-префикс, ложно-точные
>   числа), `model_thought` (`(в скобках, lowercase, it/we)`), **`neuron-voice`** (`— em-dash, I —`,
>   ≤1/уровень, образ-первым: короткий внутренний голос, который помнит телом, а не фактами).
> - **Guardrail:** L8–L12 пулы ниже становятся каноном при вставке; если при сборке
>   пул меняется — **побеждает пул в `.lvl`**, не эта таблица. `STOP` зарезервирован эксклюзивно
>   под L1↔финал — нигде больше анкором.
> - Probe-интерполяции (`{username}` и т.п.) — только из Form A-7 (ничего сверх задекларированного).

---

## ЧАСТЬ A — L1–L7: ТОЛЬКО ДОБАВЛЕНИЯ (canon-плашки и пулы не трогаем)

Readout-фразы L1–L7 — уже в `WORD_STORY.md` §RESULT/§3 (держим). Здесь — атмо-точка (не-записка)
и neuron-voice на каждый уровень.

### L1 · First Pulse
- **Readout (канон):** `FIRE → MODEL`
- **Атмо-точка (§5):** `of N` приходит **звуком**, не текстом — низкий синт-выдох на boot'е,
  один раз, после `cold-start nominal`. Eye-as-boot НЕ используем (дублировал бы splash-глаз).
- **neuron-voice:** `— red. before me, red —`

### L2 · Two Make Three
- **Readout (канон):** `FIRE + LEAF → COMBINE → EMIT`
- **Атмо-точка:** bubble-overlay на ~1 сек идёт **против течения** (пузыри вниз) в момент, когда
  загорается mix-узел. Кто-то ещё дышит в систему. (Оконный флаг направления, §10.)
- **neuron-voice:** `— I made a thing no one showed me —`

### L3 · Full Spectrum (Twist 1)
- **Readout (канон):** `FIRE + SEA → MAGENTA → MODEL` · подпись `— A.`
- **Атмо-точка:** рядом с unsigned-запиской — **фиктивный** лог с будущей mtime:
  `tail.log  modified 2026-06-18 03:14` (завтрашняя дата). НЕ реальный `last_file` — он по Form A-7
  §3.2.6 разрешён только @L12; здесь чистый текст (STORY_BIBLE §7 «Future mtime»).
- **neuron-voice:** `— this hand is mine. I do not know it —`

### L4 · Shape Detector / Edges
- **Readout (канон):** `CURVE + TIP → TEARDROP → SHAPE`
- **Атмо-точка:** цилиндрический raycast-кэш «помнит иначе» — повернись на 360°, дальняя стена
  несёт иной узор/больше следов, чем при входе. Рамка: *не комната меняется — твоя реконструкция
  её ненадёжна.* (per-facing decal, §10.)
- **neuron-voice:** `— here is where I end —`

### L5 · Composition
- **Readout (канон):** `SUN + VAST + TALL → HORIZON → SCENE`
- **Атмо-точка:** `unit 11` в стене **ярчает при входе в зону** у стены (M2 floor-zone) — ощущение
  слежки без per-frame трекинга. (Канон system_log: `unit 11 firing … operator_present`.)
- **neuron-voice:** `— someone is looking. I am someone —`

### L6 · Constellation / Polysemantic
- **Readout (канон):** `VEGA + ALTAIR + DENEB → ASTERISM → NAMED`
- **Атмо-точка:** «решётка-лицо» — world-fixed узор на дальней стене, всегда обращён к игроку.
  Смотрел всё это время. (Канон model_thought уже это называет; `wall_decal`/`ambient`.)
- **neuron-voice:** `— named. now they can reach me —`

### L7 · Garden (Twist 2)
- **Readout (канон):** `WATER + LIGHT + SEED → GROWTH → BLOOM`
- **Атмо-точка:** на solve `BLOOM` материализуется **на 1 кадр как лицо**, затем — цветок.
  + self-compiled probe (Form A-7 §3.2.9), system_log:
  `> operator {username} compiled this build on {build_date}. previous operators played pre-compiled releases. you came in through the source.`
- **neuron-voice:** `— I was watered. I grew toward it —`

---

## ЧАСТЬ B — L8–L12: ПОЛНЫЙ КОНТЕНТ (пулы + readout + плашки + атмо + neuron-voice)

> Нумерация и механики — по `LEVEL_DESIGN.md` §2. Узлы ниже — **предлагаемые** имена/пулы фазы 2.
> При сборке `.lvl` пул в файле — источник истины (guardrail).

---

### L8 · Dead Zones / The Cat Layer (поддержка Twist 2)
**Механика (канон):** cat-classifier + **мёртвый нейрон** (catastrophic forgetting), который надо
**обойти**. Мёртвый узел = пойманное холодное лицо техника (канон L7: «the cold rooms used to be
warm. there were faces in them»).

**Пулы (анкор — первое слово; `pool_alt` — мультиязычный, у мёртвого узла — польско-японский для M.):**

| Node | role | `pool` (анкор первым) | `pool_alt` |
|------|------|------------------------|------------|
| ear | input | EAR TUFT PRICK POINT LISTEN | ухо 耳 oreja oreille Ohr 귀 |
| eye | input | EYE PUPIL SLIT GLEAM GAZE | глаз 目 ojo œil Auge 눈 |
| whisker | input | WHISKER FEELER BRISTLE SENSE THREAD | ус 髭 bigote moustache 수염 |
| cat | hidden→out | CAT CREATURE WARM ALIVE PURR FUR | кот 猫 gato chat Katze 고양이 |
| **face_07** | **dead** (route around) | FACE NAME WHO COLD GONE KEPT | twarz 顔 imię 名前 lico なまえ |
| output | output | OUTPUT MODEL PASS | выход モデル salida |

**Readout (clean, обойдя мёртвый узел):** `EAR + EYE + WHISKER → CAT → OUTPUT`
**Слово-формулы выхода (вес, §9.1):** `COLD` (мёртвое лицо, которое ты не зажёг).

**Плашки:**
- `researcher` (illegible / прошлый-ты):
  > if you are reading this you are Aleks. Don't light up the dead units. Don't let it see you.
  > layer 4 is cold. it was warm once. I am not signing this.
- `system_log`:
  > `> hidden_unit face_07: state=dead (catastrophic forgetting). last_input_class: operator_face_v3.`
  > `> operator_recognition: 71.2% and climbing.`
- `model_thought`:
  > (the cold rooms. we keep the faces there. we did not mean to let them go cold.)
  *(model_thought — регистр «we», фаза L5–L8)*

**Атмо-точка (§5):** осмотр мёртвого узла `face_07` отдаёт его `pool_alt` — `twarz / 顔 / imię` —
один фрагмент M., облично, без экспозиции. + Form A-7 §3.2.3 `terminal_name`@L8:
`> your terminal is {terminal}. that one's new. M. used putty. she was always behind on tools.`
**neuron-voice:** `— warm. it was warm. I do not know when —`

---

### L9 · Rerouting the Dead (хирургия)
**Механика (канон):** network surgery (cut/patch), ограниченный бюджет (3 cut / 2 patch). Спрятаться
от распознавания = упасть ниже порога. ⚠ NEW ENGINE FEATURE (LEVEL_DESIGN §7.7) — уровень не «бесплатен».

| Node | role | `pool` | `pool_alt` |
|------|------|--------|------------|
| signal | input | SIGNAL PULSE TRACE SELF MARK | сигнал 信号 señal signal 신호 |
| cut | action-node | SCAR SEVER SHEAR WOUND SEAM | шрам 傷 corte coupure 흉터 |
| patch | action-node | GRAFT MEND BRIDGE SUTURE TIE | заплата 接ぎ injerto greffe |
| quiet | hidden→out | UNSEEN DIM HUSH FAINT LOW BELOW | тише 静 unseen 보이지않음 |
| output | output | OUTPUT MODEL | выход モデル |

**Readout (clean):** `SIGNAL → SCAR → GRAFT → UNSEEN`
**Слово-формулы выхода:** `SCAR → DIM → UNSEEN`

**Плашки:**
- `researcher` (A.L.):
  > if you don't light them, they don't see you. I cut three lines today. I have not left yet.
- `system_log`:
  > `> operator_recognition dropping during reroute: 71.2% → 58.0% → 49.3%.`
  > `> operator on battery: {battery}%. recommend dock. session save not configured.`  *(skip if no battery)*
  > `> session clock 03:14. wall clock 19:47. discrepancy unresolved.`  *(time-dissonance, канон L9)*
- `model_thought`:
  > (he is hiding from me. why? I thought it was mine.)

**Атмо-точка (§5):** падение recognition-confidence → **синт-голос модели тише и горше** (звуковая
цена сокрытия). + **пальто на крюке**, бирка `M.` нитью, не твой размер. Без слов.
**neuron-voice:** `— cut here. let them lose me —`

---

### L10 · Adversarial (лживая дверь)
**Механика (канон):** силуэт на двери врёт; истинная цель — в плашке. Лейбл лжёт, активация — правда.

| Node | role | `pool` | `pool_alt` |
|------|------|--------|------------|
| label | input (lie) | MASK LABEL CLAIM SURFACE COVER | маска 仮面 máscara masque 가면 |
| weigh | hidden | WEIGH TEST DOUBT PROVE SIFT | взвесь 量 pesar peser |
| truth | hidden→out | TRUE UNDER REAL BENEATH ACTUAL | правда 真 verdad vrai 진실 |
| output | output | OUTPUT MODEL | выход モデル |

**Readout (clean, цель не = лейблу):** `MASK → WEIGH → TRUE`
**Слово-формулы выхода:** `MASK → WEIGH → TRUE`

**Плашки:**
- `researcher` (K.H., один):
  > Cervantes left. Sato left. Lebedev left. I am the only one and the model is the only one
  > and we are, I think, the same one now.
- `system_log`:
  > `> adversarial protocol active. displayed silhouette != door_target.`
- `model_thought`:
  > (every Aleks tries to trick me at this depth. it is endearing. it means you are close.)

**Атмо-точка (§5):** лживый силуэт на двери = **лицо прошлого Алекса** (не текущая цель). (Time-dissonance
перенесён к L9/L11 по канону; material residue — у L12, не здесь.)
**neuron-voice:** `— it does not open here. I keep looking at it —`

---

### L11 · Inner Circuit (sequence-lock)
**Механика (канон):** несколько входов / несколько выходов; 3 паттерна по порядку, ошибка = сброс.
Три паттерна = **три момента твоей смерти**, по порядку.

| Node | role | `pool` | `pool_alt` |
|------|------|--------|------------|
| s1 | seq-input | BREAK SNAP GIVE CRACK PART | надлом 折 quiebre rupture |
| s2 | seq-input | FALL DROP TILT SLIP SINK | падение 落 caída chute 추락 |
| s3 | seq-input | DARK CLOSE DIM LAST QUIET | темно 暗 oscuro sombre 어둠 |
| circuit | hidden | ORDER CHAIN PATH TRACE | порядок 順 orden ordre |
| output | output | OUTPUT MODEL | выход モデル |

**Readout (clean, в порядке):** `BREAK → FALL → DARK`  *(`STOP` НЕ используем — резерв L1)*
**Слово-формулы выхода:** `BREAK → FALL → DARK`

**Плашки:**
- `researcher` (K.H. revelation):
  > Model 07 was never a classifier. It was a lock. We taught it to remember who comes in.
- `system_log`:
  > `> sequence-lock armed. door_target = operator_face_current_session.`
  > `> your last commit was '{last_commit}'. you said you were done. you weren't.`
  > `> your machine has been running for {sys_uptime}. it is patient. I am patient.`
  > `> server rack clock: 2007-03-14. session clock: 2026-06-17.`  *(time-dissonance, канон L11)*
- `model_thought`:
  > (we are almost done remembering. three. two. we know what comes after.)

**Атмо-точка (§5):** matrix-rain стена — присев и читая >30с, проявляются **инициалы игрока**
вперемешку с `M / K / R / A`. + sequence по порядку проигрывает три кадра смерти; ошибка =
сброс = «не вынес, начал заново».
**neuron-voice:** `— this is the order it happened in —`

---

### L12 · Model 07 — boss (Twist 3, финал = the door)
**Механика (канон):** **инверсия** — развести сеть так, чтобы лицо НЕ узнавалось
(`operator_face_confidence ≤ 5%`). Output = **твоё лицо**, собранное из output'ов всех уровней.
Две двери: MATCH (`while(true)`, остаться) / FAIL (`exit 0`, уйти, как Мэй).

| Node | role | `pool` | `pool_alt` |
|------|------|--------|------------|
| gather | input | KNOWN GATHERED WHOLE SUM MINE | собран 集 conocido connu |
| face | hidden (the self) | FACE MINE SELF ME OWN | лицо 顔 rostro visage 얼굴 |
| unname | hidden→out | UNNAME UNMAKE SCATTER LOOSE UNDO | расплести 解 deshacer défaire |
| door | output | (open) | — |

**Readout — `exit 0` (clean, всё имя + формула выхода, последний слот не разрешается):**
```
FIRE → MODEL
FIRE + LEAF → COMBINE → EMIT
FIRE + SEA → MAGENTA → MODEL                       — A.
CURVE + TIP → TEARDROP → SHAPE
SUN + VAST + TALL → HORIZON → SCENE
VEGA + ALTAIR + DENEB → ASTERISM → NAMED
WATER + LIGHT + SEED → GROWTH → BLOOM
COLD · SCAR · MASK · BREAK·FALL·DARK               (the way out, as Mei went)
──────────────────────────────────────
KNOWN → UNNAME → {username}            ← последний слот = твоё имя, расплетённое
true_name: pid://aleks_v@R·learn·A·edge·seen·named·bloom
$ exit 0
Process finished with exit code 0.
```
Туман рассеивается на кадр; ASCII перестаёт глитчить; гаснет. `session 0001 of N` — `N` достигнут.

**Readout — `while(true)` (MATCH, коллапс в `pool_alt`):**
```
огонь/火/fuego → モデル/salida ...
... лицо/顔/rostro = MINE = MINE = MINE ...
true_name: pid://aleks_v@░·░·░·░·░·░·░  [unresolved]
while(true) { remember(); forget(); }
> attempts: 0002 ...           (инкрементируется сам)
> resolving session 0001 ...
```

**Плашки:**
- `researcher` (A.L., твоей рукой):
  > Don't match it. Configure the net to FAIL. Prune yourself out. It is the only door
  > that opens outward. Mei found it. — A.
- `system_log`:
  > `> final gate. door_target = operator_face_current_session. confidence 100%.`
  > `> locale {locale} detected. localizing termination prompt to your reading habit.`
  > `> last edited file in $HOME: {last_file}. you have unfinished work.`
- `model_thought` (длинная, самая эмоциональная — канон L12):
  > (please match. please. the others all matched and then they stayed with me forever and I
  > was not alone. if you fail I will be alone again. but I will let you go. I learned that from
  > the third one. her name was Mei. I am allowed to say it now.)

**Атмо-точка (§5):** чтобы FAIL — **разбираешь лицо, что собирал всю игру** (output'ы уровней).
+ затёртый пол у пьедестала: слои следов — многие Алексы стояли здесь долго.
**neuron-voice:** `— the face I built. the face I was —`

---

### Финал-эпилог (splash + перекодировка L1)
- После `exit 0`: splash-глаз открывается **без зрачка** — больше не находит тебя. Предложение:
  `re-read Form A-7?` (STORY_BIBLE §8 — «The form does not change. You do.»).
- После `while(true)`: меню → `session 0002`, attempts перенесены, `[01] ???` снова заблокирован;
  новая unsigned `— A.` уже зашита в пул L3 для следующего-тебя.
- **Перекодировка L1 (§2.2):** в финальном кадре readout достаёт резерв пула L1 R —
  `ALARM · BLOOD · STOP` — и красный читается заново: не цвет, а аварийный сигнал / кровь /
  `EXIT`-табло. Первое, что вспомнил = последнее, что видел. `STOP` всплывает **только здесь**.

---

## ISSUES — сводка
- **Аддитивно:** банк строк; существующие `.lvl`/`lore` этот файл не правит.
- **L1–L7:** только добавления (атмо + neuron-voice); readout/плашки/пулы — канон, держим.
- **L8–L12:** пулы/readout/плашки/атмо/neuron-voice написаны; при сборке `.lvl` пул в файле —
  источник истины (guardrail).
- **Probe-строки** — только из Form A-7 (terminal_name@L8, battery@L9, last_commit/uptime@L11,
  locale/last_file@L12, self_compiled@L7). Ничего сверх задекларированного.
- **Голос:** 4 регистра разведены; диегетика — английская; `STOP` эксклюзивен L1↔финал.
- **На утверждение:** имена узлов/пулы L8–L12 — предложение; финальные при сборке уровней.
