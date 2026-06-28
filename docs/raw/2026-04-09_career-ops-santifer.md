# Career-Ops — AI Job Search Pipeline (santifer)

> Source: https://github.com/santifer/career-ops (D:\ressources externes\career-ops-main)
> Ingéré: 2026-04-09

## Ce que c'est
Pipeline IA de recherche d'emploi : 740+ offres évaluées, 100+ CVs générés, scoring A-F, batch parallel.
Construit sur Claude Code + Node.js + Playwright.

## 12 Patterns architecturaux identifiés

### 1. Data Contract (User Layer / System Layer)
Séparation stricte entre données utilisateur (jamais auto-updated) et données système (safe to update).

### 2. Modes System (Prompt-Based Routing)
Chaque workflow = un `.md` auto-contenu. Router dans SKILL.md dispatch intent → mode.

### 3. Batch Workers (Headless Parallelism)
`claude -p` workers indépendants + state file TSV (pending/completed/failed) + merge phase.
Resumable, error-isolated, output normalisé.

### 4. Tracker Integrity (Dedup + Normalize + Verify)
Scripts JS: merge-tracker (fuzzy dedup company+role), verify-pipeline (health check), normalize-statuses (aliases → canonical), dedup-tracker.

### 5. Pattern Analysis (Parse → Classify → Recommend)
Analyse rétrospective: parse reports → classifier (outcome, remote, size, gaps) → funnel stats → recommandations actionnables.

### 6. Health Check (doctor.mjs)
Checklist validation: prerequisites, fichiers requis, dépendances, directories. Exit code 0/1.

### 7. YAML Config (Single Source of Truth)
profile.yml centralise identité, cibles, narrative, compensation, location.

### 8. Skill Router (Intent-Based Dispatch)
SKILL.md parse input → detect intent → load context (shared + specific + language) → execute.

### 9. PDF Generation (ATS Normalization)
HTML → normalize Unicode (em-dashes, smart quotes, zero-width) → Playwright render → PDF.

### 10. Dashboard (Go TUI)
Application Go standalone lisant les mêmes .md que l'IA. Filtrage, tri, édition inline.

### 11. Canonical States
states.yml définit les statuts valides + aliases. Scripts normalisent automatiquement.

### 12. Markdown Pipeline
Tout le flux passe par markdown: source → report → PDF → tracker → dashboard.

## Ce qu'on a adopté pour Forgia
1. **Tracker Integrity** → `/verify-stories` (lint stories)
2. **Pattern Analysis** → `/analyze-patterns` (rétrospective structurée)
3. **Batch Workers** → `/batch-workers` (orchestration parallèle N agents)
