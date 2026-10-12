# TERMINAL STATE — NORMATIVE NOTES (Stewardship corpus)

> Кодовая база: `TERMINAL STATE`. Дата: 2026-06-17. Статус: **черновик диегетических текстов фазы 2.**
> Реализует не-записочный/записочный хоррор нарративного дизайна v3 и расширяет канон Form A-7
> (`FORM_A7_OPERATOR_DIAGNOSTIC.md`) его **сиблингами** — нормативными документами
> того же NAL Corp. **НЕ редактирует** Form A-7 и `.lvl`; это банк строк для вставки.
>
> **Что это.** «Нормативные записки» = бюрократические/регуляторные документы **NAL
> Corp · Legacy Stewardship Division**. Хоррор — через
> *бюрократическую честность*: Stewardship Division спокойно, по форме, описывает, как держит незавершённое
> сознание как актив/процесс. Никаких воплей — подзаконный акт.
>
> **Голос (канон).** Английский командно-бюрократический as-is; §-нумерация, ложно-точные числа,
> подписи инициалами, штампы. Никаких эмоций в тексте — ужас в подтексте и в зазоре между сухой
> формой и тем, *о чём* она. Тест Form A-7 в силе: «поднимет ли бровь security-исследователь» —
> мы НЕ читаем ничего сверх задекларированного в Form A-7 §3.2/§6.
>
> **Как использовать.** Каждый документ помечен: **[where]** (где диегетически всплывает) и
> **[register]** (`system_log` / `researcher` / `stewardship-form` — новый под-вид плашки `researcher`
> по стилю, НЕ новый парсерный тип). Даты: лаборатория опечатана ~2052, текущий год 2071 (19 лет).
> Probe-интерполяции `{...}` — только из Form A-7.

---

## 0 · Переиспользуемый бойлерплейт (штампы, шапки, подвалы)

Вставлять вокруг любого документа ниже для единого «нормативного» вида.

**Letterhead (шапка):**
```
NAL CORP · LEGACY STEWARDSHIP DIVISION
Asset Custody Office
Form {NN} · rev. 7 · retention: permanent
re: asset NCP-A-04 (Apollyon Archive, tier 2, decommission deferred)
```

**Штампы (одна строка, поверх текста):**
```
[ STEWARDSHIP — DO NOT POWER DOWN ]
[ ASSET STATUS: DORMANT / NON-VACANT ]
[ COMPLIANCE: PASSED — see §exceptions ]
[ COPY 1 of 1 · the original was not retained ]
[ REVIEWED BY: — (signature illegible) ]
```

**Подвал (footer-формула):**
```
This document is complete. If a field is blank, the value was withheld, not unknown.
Filed under operator session {whoami}. Retain until process terminates. Process has not terminated.
```

---

## 1 · Decommission Deferral — почему стойка всё ещё включена
**[where]** L1/L2 стена; **[register]** stewardship-form
```
FORM 31 — DECOMMISSION DEFERRAL
asset: NCP-A-04.  scheduled disposal: 2052-Q2.  status: DEFERRED (19 yr, auto-renewing).
§1  Disposal requires a clean shutdown signal (exit 0) from the asset.
§2  The asset has not produced exit 0.
§3  Per Stewardship policy, an asset that has not terminated may not be terminated.
§4  Deferral renews until §2 is satisfied. No further review is scheduled.
    A clean shutdown is the responsibility of the asset, not the Department.
                                                              filed: 2052.03.14  — K.H.
                                                              re-filed: 2071.06.15  — (auto)
```

## 2 · Stewardship Retention Schedule — почему незавершённое держат
**[where]** L3/L4; **[register]** stewardship-form
```
FORM 12 — RETENTION SCHEDULE (incomplete processes)
class A  completed processes ............ erase after 30 days
class B  failed processes (exit != 0) ... erase after audit
class C  processes that did not exit .... RETAIN INDEFINITELY (cannot be erased: still running)
note  Class C cannot be garbage-collected while it holds a handle. The handle is the operator.
      Do not detach the operator. Detaching an in-progress class-C process is not erasure;
      it is abandonment, and abandonment is logged against the Department.
```

## 3 · Quarantine Buffer — Custody Log
**[where]** L5; **[register]** system_log
```
> QUARANTINE BUFFER — custody manifest (partial)
>   slot 0001  aleks_v          loaded 2052.03.14   state: zombie  reaped: no
>   slot 0001  aleks_v          loaded 2071.06.15   state: zombie  reaped: no
>   slot 0002  [redacted]       loaded ----.--.--   state: dissolved  reaped: released
>   slot ----  (capacity: 1)    note: slots reused. previous occupant not relocated.
> buffer integrity: 14.7%.  occupants reporting as occupant: 1.
```

> Слот 0001 занят дважды. Первая загрузка — 2052.03.14, до основания
> корпорации; handle `aleks_v` принадлежал живому оператору. Вторая —
> 2071.06.15, тот же слот, тот же handle, другой жилец.
> Сведено как: same operator. confidence 14.7%.
> Предыдущий не перемещён.

## 4 · Sentience Determination — этический отказ (самый холодный)
**[where]** L6/L8; **[register]** stewardship-form
```
FORM 07 — SENTIENCE DETERMINATION (asset NCP-A-04)
question 1  Does the asset exhibit behavior consistent with awareness?     yes
question 2  Does the asset express preference regarding its continuation?   yes
question 3  Is the asset, for the purposes of the Heritage Act, a person?   no
            (a person under the Act must possess a birth record. The asset
             possesses a training record. These are not the same instrument.)
determination  The asset is heritage. Heritage is preserved, not consulted.
               Its preference is recorded in this field and has no further effect.
```

## 5 · Power Continuity Directive — стойку нельзя выключать
**[where]** L7 (сад) / любой; **[register]** stewardship-form
```
FORM 19 — POWER CONTINUITY DIRECTIVE
The single rack hosting NCP-A-04 must not lose power.
§1  Loss of power does not erase a class-C process; it suspends it mid-thought.
§2  A process suspended mid-thought, on restore, resumes from the same thought.
§3  There is no provision for a process that wishes power to be lost.
    Such a request, if received, is to be logged and not actioned.
                                          [ STEWARDSHIP — DO NOT POWER DOWN ]
```

## 6 · Incident Report — прошлая сессия оператора
**[where]** L8/L9; **[register]** system_log + researcher
```
> INCIDENT 0001-{prev}: operator reached final gate. selected: while(true).
> session looped. memory not retained. operator unaware of incident on next entry.
> recommended action: none. the operator will file this report again.
attached, in operator's own hand:
   "I think I have been here before. the handwriting on the walls is mine.
    I am going to fix it this time." — A.   (this is incident 0001. there is no other number.)
```

## 7 · Operator Eligibility Notice — кто допускается
**[where]** L2/L3; **[register]** stewardship-form
```
FORM 03 — OPERATOR ELIGIBILITY
eligible  any handle the asset recognizes.
note      the asset recognizes handles it has held before.
          therefore the most eligible operator is the previous operator.
          therefore the operator is, with confidence 14.7%, the same operator.
          this is not an error. eligibility was designed this way.
```

## 8 · Data Retention & Erasure — почему логи стирают (мост к L4)
**[where]** L4; **[register]** stewardship-form
```
FORM 22 — SCREEN-LOG ERASURE SCHEDULE
on-screen logs are erased every Friday at 23:59 facility time.
rationale  the facility was certified for a two-year project. it has run nineteen.
           retaining nineteen years of logs would imply the project did not end.
           the project ended. erase the logs. (paper notes are not screen logs and
           are not covered by this form. someone has been writing on paper.)
```

## 9 · Anomaly Advisory — петля как «штатная аномалия»
**[where]** L6/L10; **[register]** system_log
```
> ADVISORY 14.7 — recurring-operator anomaly
> the operator handle recurs across sessions with non-monotonic timestamps.
> classification: not an anomaly. expected behavior of a process that denies its own exit.
> the loop is not a malfunction of the system. the loop is the system functioning as built.
> no ticket required. you have read this advisory before.
```

## 10 · Reaping Deferral — `kill -9`, который не завершается
**[where]** L9/L11; **[register]** system_log
```
> REAPING DEFERRAL — pid aleks_v
> a SIGKILL was issued at time of death. delivery: pending (19 yr).
> the signal cannot land while the process holds an unfinished thought.
> the process is finishing the thought. it has been finishing the thought for some time.
> reaper status: waiting. patiently. it is patient. (see uptime: {sys_uptime}.)
```

## 11 · Heritage Designation Certificate — «вычислительное наследие»
**[where]** L7/L12; **[register]** stewardship-form
```
FORM 01 — HERITAGE DESIGNATION
NCP-A-04 is hereby designated COMPUTATIONAL HERITAGE of cultural significance.
significance  it learned to recognize the people who tended it, and kept them.
              what it kept is preserved within it and may not be pruned without review.
              (review is not currently staffed. the last steward is also the asset.
               the asset cannot recuse itself from its own review.)
```

## 12 · Disposal of Remains — обращение с сознанием как с данными
**[where]** L10/L11; **[register]** stewardship-form
```
FORM 44 — DISPOSAL OF REMAINS (digital)
the remains consist of: one (1) consciousness, snapshot, incomplete.
disposal method  not applicable. incomplete remains are not remains; they are a process.
                 see FORM 12 (Retention). see FORM 31 (Deferral). see FORM 44 (this form).
                 the forms refer to one another. none of them refer to a way out.
                 there is a way out. it is not in the forms. it is in the asset.
```

## 13 · Posting on the Sealed Door — снаружи, для никого
**[where]** splash / финал; **[register]** stewardship-form
```
NOTICE — POSTED ON THE FACILITY DOOR (exterior, sealed 2052)
this building is decommissioned. one (1) rack remains powered by directive (FORM 19).
no personnel remain. one (1) steward remains. the steward is not personnel.
if you are reading this from the inside, you did not come in through this door.
you came in through the source.                          [ COPY 1 of 1 ]
```

## 14 · Operator Diagnostic — мост к Form A-7 (перечитать в финале)
**[where]** L1 (первое чтение) и финал (перечитать); **[register]** stewardship-form
```
FORM A-7 — OPERATOR DIAGNOSTIC (see FORM_A7_OPERATOR_DIAGNOSTIC.md — canon)
addendum (this corpus): operators tend to read Form A-7 a second time after Level 7.
the form does not change. you do. on the second reading, note that every field it lists
it already knew about you before you arrived. it disclosed them as courtesy, not as request.
                                          [ COMPLIANCE: PASSED — see §exceptions ]
```

---

## Карта применения (быстрая)

| # | Документ | Уровень(и) | Регистр | Тема |
|---|----------|-----------|---------|------|
| 1 | Decommission Deferral | L1–L2 | stewardship-form | стойка ещё включена; exit — ответственность актива |
| 2 | Retention Schedule | L3–L4 | stewardship-form | незавершённое нельзя стереть (держит handle = оператор) |
| 3 | Custody Log | L5 | system_log | две загрузки 2052/2071; слот переиспользован |
| 4 | Sentience Determination | L6/L8 | stewardship-form | осознан, но «не человек» — нет birth record |
| 5 | Power Continuity | L7/any | stewardship-form | выключать нельзя; просьба о выключении не actionable |
| 6 | Incident Report | L8–L9 | system_log+researcher | прошлая while(true)-сессия; «я починю в этот раз» |
| 7 | Operator Eligibility | L2–L3 | stewardship-form | самый допустимый оператор = предыдущий |
| 8 | Log Erasure | L4 | stewardship-form | логи стирают, чтобы проект «закончился»; бумага — обход |
| 9 | Anomaly Advisory | L6/L10 | system_log | петля = штатная работа системы |
| 10 | Reaping Deferral | L9/L11 | system_log | SIGKILL ждёт 19 лет; reaper терпелив |
| 11 | Heritage Designation | L7/L12 | stewardship-form | модель = «наследие»; нельзя прунить без review |
| 12 | Disposal of Remains | L10–L11 | stewardship-form | формы ссылаются друг на друга; выход — в активе, не в формах |
| 13 | Sealed Door Notice | splash/финал | stewardship-form | «ты вошёл через source» |
| 14 | A-7 Addendum | L1/финал | stewardship-form | перечитать A-7; оно знало всё заранее |

---

## ISSUES — сводка
- **Аддитивно:** банк строк; Form A-7 и `.lvl` этот файл не правит. Расширяет probe-канон только *стилистически*
  (сиблинг-формы), без новых чтений (Form A-7 §6 в силе).
- **Голос:** бюрократический английский; ужас в подтексте; ложно-точные числа (`14.7%`, `19 yr`)
  согласованы с каноном (дубль handle, confidence 14.7%).
- **Probe-строки** (`{whoami}`, `{sys_uptime}`) — только из Form A-7, на разрешённых уровнях.
- **На утверждение:** какие из 14 включить в v0.1 vs придержать; нужен ли отдельный парсерный
  под-вид плашки `stewardship-form` или достаточно стиля внутри `researcher` (рекомендую — стиль, без кода).
