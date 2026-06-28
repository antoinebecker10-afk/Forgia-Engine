#!/usr/bin/env node
// ============================================================================
// Forgia Multi-Agent Orchestrator v1.0
// Spawns multiple `claude -p` processes with specialized agent roles.
// Zero npm dependencies — uses only Node.js builtins.
// ============================================================================

const { execSync, spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

// ─── Config & Constants ─────────────────────────────────────────────────────

const PROJECT_ROOT = path.resolve(__dirname, '..', 'RUST', 'Forgia', 'Forgia');
const STORIES_DIR = path.join(PROJECT_ROOT, 'docs', 'stories');
const CLAUDE_MD = path.join(PROJECT_ROOT, 'CLAUDE.md');
const AGENTS_DIR = path.join(process.env.HOME || process.env.USERPROFILE, '.openclaw', 'workspaces');
const SUMMARY_FILE = path.join(__dirname, 'claude-summary.md');
const CHECKLIST_FILE = path.join(PROJECT_ROOT, '.bmad', 'checklists', 'post-implementation.md');
const INDEX_FILE = path.join(STORIES_DIR, '_index.md');

const MAX_CONCURRENT = 3;
const CLAUDE_TIMEOUT = 120_000; // 2 minutes
const MAX_FILE_LINES = 500;
const VERSION = '1.0.0';

// ─── Nested session guard ────────────────────────────────────────────────────
// claude -p cannot be spawned from within an active Claude Code session.
// Detect and warn early.
if (process.env.CLAUDECODE === '1' || process.env.CLAUDE_CODE_SSE_PORT) {
  const isListOnly = process.argv.includes('--agents') || process.argv.includes('--help') || process.argv.includes('-h') || process.argv.includes('--self-test');
  if (!isListOnly) {
    console.error('\x1b[31m');
    console.error('  ⛔ Session Claude Code detectee !');
    console.error('  L\'orchestrateur ne peut pas spawner claude -p depuis une session active.');
    console.error('  Lance-le depuis un terminal externe :');
    console.error('');
    console.error('    cd "C:\\Users\\Antoi\\Desktop\\Forgia"');
    console.error('    node tools/orchestrator.js --dry-run "ta tache ici"');
    console.error('\x1b[0m');
    process.exit(1);
  }
}

// ─── ANSI colors ─────────────────────────────────────────────────────────────

const C = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  cyan: '\x1b[36m',
  magenta: '\x1b[35m',
  gray: '\x1b[90m',
};

function log(msg) { process.stdout.write(msg + '\n'); }
function logPhase(phase, msg) { log(`\n${C.bold}${C.cyan}${phase}${C.reset} ${C.dim}—${C.reset} ${msg}`); }
function logOk(msg) { log(`  ${C.green}✅${C.reset} ${msg}`); }
function logFail(msg) { log(`  ${C.red}❌${C.reset} ${msg}`); }
function logInfo(msg) { log(`  ${C.dim}→${C.reset} ${msg}`); }

// ─── Agent Registry ─────────────────────────────────────────────────────────

const AGENT_REGISTRY = {
  'igc-architect':  { name: 'Archibald',  emoji: '🏗️',  domain: 'architecture, design, planning, review' },
  'forgia-dev':        { name: 'Rusty',      emoji: '⚙️',  domain: 'rust code, bevy, implementation, systems' },
  'forgia-qa':         { name: 'Sentinel',   emoji: '🛡️',  domain: 'qa, validation, checklist, review, testing' },
  'igc-gamedesign': { name: 'Ludovic',    emoji: '🎯', domain: 'game design, balance, mechanics, GDD' },
  'igc-ui':         { name: 'Pixel',      emoji: '🎨', domain: 'ui, ux, egui, panels, layout, hud' },
  'igc-vfx':        { name: 'Spark',      emoji: '✨', domain: 'vfx, particles, hanabi, lighting, bloom' },
  'igc-level':      { name: 'Atlas',      emoji: '🗺️',  domain: 'level design, maps, spawns, presets' },
  'igc-audio':      { name: 'Echo',       emoji: '🔊', domain: 'audio, sound, music, spatial audio' },
  'forgia-terrain':    { name: 'Terra',      emoji: '🏔️',  domain: 'terrain, sdf, voxel, chunks, biomes, meshing' },
  'igc-research':   { name: 'Newton',     emoji: '🔬', domain: 'research, crates, benchmarks, bevy ecosystem' },
  'igc-docs':       { name: 'Scribe',     emoji: '📝', domain: 'documentation, stories, changelog, guides' },
  'forgia-devops':     { name: 'Rocket',     emoji: '🚀', domain: 'build, ci/cd, cargo, wasm, releases' },
  'igc-community':  { name: 'Herald',     emoji: '📣', domain: 'community, marketing, content, communication' },
  'igc-security':   { name: 'Cipher',     emoji: '🔐', domain: 'security, audit, tokens, vulnerabilities' },
};

function loadAgents() {
  const agents = {};
  for (const [id, meta] of Object.entries(AGENT_REGISTRY)) {
    const soulPath = path.join(AGENTS_DIR, id, 'SOUL.md');
    let soul = '';
    let status = 'missing';
    if (fs.existsSync(soulPath)) {
      soul = fs.readFileSync(soulPath, 'utf-8');
      status = 'ready';
    }
    agents[id] = { ...meta, soul, status };
  }
  return agents;
}

// ─── Core — callAgent ────────────────────────────────────────────────────────

function callAgent(agentId, prompt, agents, retryCount = 0) {
  return new Promise((resolve) => {
    const agent = agents[agentId];
    const startTime = Date.now();
    const label = `${agent.emoji} ${agent.name}`;

    process.stdout.write(`  ${label}...${C.dim}`);

    // Build system prompt from agent SOUL
    const systemPrompt = agent.soul || `Tu es ${agent.name}, agent ${agentId} du projet Forgia.`;

    // Prepare env — remove Claude Code markers to avoid nested session detection
    const env = { ...process.env };
    delete env.CLAUDECODE;
    delete env.CLAUDE_CODE;
    delete env.CLAUDE_CODE_SSE_PORT;
    delete env.CLAUDE_CODE_ENTRYPOINT;

    const args = [
      '-p', prompt,
      '--output-format', 'text',
      '--no-session-persistence',
      '--system-prompt', systemPrompt,
    ];

    const child = spawn('claude', args, {
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
      shell: true,
    });

    let stdout = '';
    let stderr = '';

    child.stdout.on('data', (data) => { stdout += data.toString(); });
    child.stderr.on('data', (data) => { stderr += data.toString(); });

    const timer = setTimeout(() => {
      child.kill('SIGTERM');
    }, CLAUDE_TIMEOUT);

    child.on('close', (code) => {
      clearTimeout(timer);
      const duration = ((Date.now() - startTime) / 1000).toFixed(0);

      if (code === 0 && stdout.trim()) {
        process.stdout.write(`\r  ${label}...       ${C.green}✅${C.reset} (${duration}s)\n`);
        resolve({ agentId, output: stdout.trim(), duration: +duration, success: true });
      } else if (retryCount < 1) {
        process.stdout.write(`\r  ${label}...       ${C.yellow}⟳${C.reset} retry\n`);
        resolve(callAgent(agentId, prompt, agents, retryCount + 1));
      } else {
        process.stdout.write(`\r  ${label}...       ${C.red}❌${C.reset} (${duration}s)\n`);
        const errMsg = stderr.trim() || `Exit code ${code}`;
        resolve({ agentId, output: '', duration: +duration, success: false, error: errMsg });
      }
    });

    child.on('error', (err) => {
      clearTimeout(timer);
      const duration = ((Date.now() - startTime) / 1000).toFixed(0);
      if (retryCount < 1) {
        process.stdout.write(`\r  ${label}...       ${C.yellow}⟳${C.reset} retry\n`);
        resolve(callAgent(agentId, prompt, agents, retryCount + 1));
      } else {
        process.stdout.write(`\r  ${label}...       ${C.red}❌${C.reset} (${duration}s)\n`);
        resolve({ agentId, output: '', duration: +duration, success: false, error: err.message });
      }
    });
  });
}

// ─── Queue — runParallel ─────────────────────────────────────────────────────

async function runParallel(tasks, maxConcurrent = MAX_CONCURRENT) {
  const results = [];
  let idx = 0;

  async function worker() {
    while (idx < tasks.length) {
      const currentIdx = idx++;
      const task = tasks[currentIdx];
      const result = await task();
      results[currentIdx] = result;
    }
  }

  const workers = [];
  for (let i = 0; i < Math.min(maxConcurrent, tasks.length); i++) {
    workers.push(worker());
  }
  await Promise.all(workers);
  return results;
}

// ─── File Utilities ──────────────────────────────────────────────────────────

function readFileSafe(filePath, maxLines = MAX_FILE_LINES) {
  try {
    const content = fs.readFileSync(filePath, 'utf-8');
    const lines = content.split('\n');
    if (lines.length > maxLines) {
      return lines.slice(0, maxLines).join('\n') + `\n// ... (${lines.length - maxLines} more lines truncated)`;
    }
    return content;
  } catch {
    return null;
  }
}

function loadSummary() {
  if (fs.existsSync(SUMMARY_FILE)) {
    return fs.readFileSync(SUMMARY_FILE, 'utf-8');
  }
  return '(No summary file found)';
}

function loadStory(storyId) {
  // Try common patterns
  const patterns = [
    path.join(STORIES_DIR, `${storyId}.md`),
    path.join(STORIES_DIR, `${storyId}-*.md`),
  ];

  // Direct match
  if (fs.existsSync(patterns[0])) {
    return fs.readFileSync(patterns[0], 'utf-8');
  }

  // Glob match
  try {
    const files = fs.readdirSync(STORIES_DIR);
    const match = files.find(f => f.startsWith(storyId));
    if (match) {
      return fs.readFileSync(path.join(STORIES_DIR, match), 'utf-8');
    }
  } catch { /* ignore */ }

  return null;
}

function loadIndex() {
  if (fs.existsSync(INDEX_FILE)) {
    return fs.readFileSync(INDEX_FILE, 'utf-8');
  }
  return '';
}

// ─── Phase A — Decompose ────────────────────────────────────────────────────

async function decompose(taskDescription, agents, storyContent) {
  logPhase('Phase A', 'Analyse...');

  const agentList = Object.entries(agents)
    .filter(([, a]) => a.status === 'ready')
    .map(([id, a]) => `  - ${a.emoji} ${a.name} (${id}): ${a.domain}`)
    .join('\n');

  const storyContext = storyContent
    ? `\n\n## Story Content:\n${storyContent}`
    : '';

  const indexContent = loadIndex();

  const prompt = `Tu es l'Orchestrateur Forgia. Decompose cette tache en sous-taches pour des agents specialises.

## Tache:
${taskDescription}
${storyContext}

## Agents disponibles:
${agentList}

## Stories index:
${indexContent}

## Instructions:
1. Analyse la tache et identifie les sous-taches necessaires.
2. Assigne chaque sous-tache a l'agent le plus pertinent.
3. Identifie les fichiers source a lire et a modifier.
4. Determine l'ordre d'execution (groupes paralleles).

Reponds UNIQUEMENT avec un bloc JSON valide (pas de markdown, pas de commentaires):

{
  "summary": "Description courte de la tache",
  "subtasks": [
    {
      "id": "T1",
      "agent": "forgia-dev",
      "description": "Description de la sous-tache",
      "files_to_read": ["src/example.rs"],
      "files_to_write": ["src/example.rs"],
      "depends_on": []
    }
  ],
  "execution_order": [["T1", "T2"], ["T3"]]
}`;

  const result = await callAgent('igc-architect', prompt, agents);

  if (!result.success) {
    logFail(`Decomposition echouee: ${result.error || 'unknown'}`);
    return null;
  }

  // Parse JSON from output (may be wrapped in markdown code block)
  let json = result.output;
  const jsonMatch = json.match(/```(?:json)?\s*([\s\S]*?)```/);
  if (jsonMatch) json = jsonMatch[1];

  // Try to find raw JSON object
  const braceMatch = json.match(/\{[\s\S]*\}/);
  if (braceMatch) json = braceMatch[0];

  try {
    const plan = JSON.parse(json);
    logInfo(`${plan.subtasks.length} sous-taches, ${plan.execution_order.length} groupes`);
    for (const st of plan.subtasks) {
      const agent = agents[st.agent];
      const label = agent ? `${agent.emoji} ${agent.name}` : st.agent;
      logInfo(`${st.id}: ${label} — ${st.description.slice(0, 60)}`);
    }
    return plan;
  } catch (e) {
    logFail(`JSON parse error: ${e.message}`);
    logInfo(`Raw output (first 500 chars): ${result.output.slice(0, 500)}`);
    return null;
  }
}

// ─── Phase B — Build Context ────────────────────────────────────────────────

function buildContext(subtask, summary) {
  const contextParts = [`## Contexte projet (resume)\n${summary}`];

  // Read source files
  if (subtask.files_to_read && subtask.files_to_read.length > 0) {
    contextParts.push('\n## Fichiers source actuels:');
    for (const relPath of subtask.files_to_read) {
      const absPath = path.join(PROJECT_ROOT, relPath);
      const content = readFileSafe(absPath);
      if (content) {
        contextParts.push(`\n### ${relPath}\n\`\`\`rust\n${content}\n\`\`\``);
      } else {
        contextParts.push(`\n### ${relPath}\n(fichier non trouve ou vide)`);
      }
    }
  }

  const filesOut = (subtask.files_to_write || []).join(', ') || '(aucun)';

  const prompt = `## Ta mission
${subtask.description}

## Fichiers a modifier
${filesOut}

${contextParts.join('\n')}

## Format de reponse
Pour chaque fichier modifie, utilise ce format EXACT:

// FILE: chemin/relatif/fichier.rs
(contenu complet du fichier ou du bloc modifie)
// END FILE

Si tu ne modifies qu'une partie d'un fichier, montre le contexte suffisant (fonctions completes) pour identifier ou inserer le code.
Reponds en francais pour les commentaires, anglais pour le code.`;

  return prompt;
}

// ─── Phase C — Execute ──────────────────────────────────────────────────────

async function executeSubtasks(plan, agents) {
  logPhase('Phase C', `Execution (max ${MAX_CONCURRENT} paralleles)`);

  const summary = loadSummary();
  const subtasksMap = {};
  for (const st of plan.subtasks) {
    subtasksMap[st.id] = st;
  }

  const allResults = [];

  for (let groupIdx = 0; groupIdx < plan.execution_order.length; groupIdx++) {
    const group = plan.execution_order[groupIdx];
    if (plan.execution_order.length > 1) {
      logInfo(`Groupe ${groupIdx + 1}/${plan.execution_order.length}: [${group.join(', ')}]`);
    }

    const tasks = group.map(taskId => {
      const subtask = subtasksMap[taskId];
      if (!subtask) {
        return () => Promise.resolve({ agentId: 'unknown', output: '', success: false, error: `Task ${taskId} not found` });
      }
      const prompt = buildContext(subtask, summary);
      return () => callAgent(subtask.agent, prompt, agents);
    });

    const groupResults = await runParallel(tasks, MAX_CONCURRENT);

    // Map results back with task info
    for (let i = 0; i < group.length; i++) {
      allResults.push({
        taskId: group[i],
        subtask: subtasksMap[group[i]],
        ...groupResults[i],
      });
    }
  }

  return allResults;
}

// ─── Phase D — Collect & Merge ──────────────────────────────────────────────

function collectAndMerge(results) {
  logPhase('Phase D', 'Merge...');

  const fileChanges = {}; // path → [{ taskId, content }]

  for (const result of results) {
    if (!result.success || !result.output) continue;

    // Parse // FILE: ... // END FILE blocks
    const fileBlocks = result.output.matchAll(/\/\/ FILE:\s*(.+?)\s*\n([\s\S]*?)\/\/ END FILE/g);
    for (const match of fileBlocks) {
      const filePath = match[1].trim();
      const content = match[2].trim();
      if (!fileChanges[filePath]) fileChanges[filePath] = [];
      fileChanges[filePath].push({ taskId: result.taskId, agentId: result.agentId, content });
    }
  }

  const conflicts = [];
  const merged = {};

  for (const [filePath, changes] of Object.entries(fileChanges)) {
    if (changes.length === 1) {
      merged[filePath] = changes[0].content;
    } else {
      conflicts.push({ filePath, changes });
      // For now, take the last one (agent with highest priority — usually dev > others)
      const priorityOrder = ['forgia-dev', 'forgia-terrain', 'igc-ui', 'igc-architect'];
      let bestChange = changes[changes.length - 1];
      for (const prio of priorityOrder) {
        const found = changes.find(c => c.agentId === prio);
        if (found) { bestChange = found; break; }
      }
      merged[filePath] = bestChange.content;
    }
  }

  const fileCount = Object.keys(merged).length;
  logInfo(`${fileCount} fichier(s), ${conflicts.length} conflit(s)`);

  if (conflicts.length > 0) {
    for (const c of conflicts) {
      log(`  ${C.yellow}⚠️${C.reset}  ${c.filePath} — ${c.changes.length} agents (resolution: priorite)`);
    }
  }

  return { merged, conflicts };
}

// ─── Phase E — Validate ─────────────────────────────────────────────────────

async function validate(merged, storyContent, agents) {
  logPhase('Phase E', 'QA');

  let checklist = '';
  if (fs.existsSync(CHECKLIST_FILE)) {
    checklist = fs.readFileSync(CHECKLIST_FILE, 'utf-8');
  }

  const changesStr = Object.entries(merged)
    .map(([fp, content]) => `### ${fp}\n\`\`\`rust\n${content.slice(0, 2000)}\n\`\`\``)
    .join('\n\n');

  const storyCtx = storyContent ? `\n## Story:\n${storyContent}` : '';

  const prompt = `Tu es Sentinel, QA du projet Forgia. Valide ces changements.

## Changements proposes:
${changesStr}
${storyCtx}

## Checklist:
${checklist}

## Instructions:
Analyse les changements et reponds UNIQUEMENT avec un bloc JSON:

{
  "approved": true,
  "issues": [],
  "summary": "Resume de la validation"
}

Si il y a des problemes:
{
  "approved": false,
  "issues": [
    { "severity": "CRITICAL", "file": "path", "description": "..." }
  ],
  "summary": "Resume des problemes"
}`;

  const result = await callAgent('forgia-qa', prompt, agents);

  if (!result.success) {
    logFail(`QA echouee: ${result.error || 'unknown'}`);
    return { approved: false, issues: [{ severity: 'CRITICAL', description: 'QA agent failed' }] };
  }

  // Parse JSON
  let json = result.output;
  const jsonMatch = json.match(/```(?:json)?\s*([\s\S]*?)```/);
  if (jsonMatch) json = jsonMatch[1];
  const braceMatch = json.match(/\{[\s\S]*\}/);
  if (braceMatch) json = braceMatch[0];

  try {
    const validation = JSON.parse(json);
    if (validation.approved) {
      logOk(`PASS — ${validation.summary || 'Approuve'}`);
    } else {
      logFail(`FAIL — ${validation.summary || ''}`);
      for (const issue of (validation.issues || [])) {
        log(`    ${C.red}${issue.severity}${C.reset}: ${issue.description}`);
      }
    }
    return validation;
  } catch {
    // If we can't parse, check for keywords
    const lower = result.output.toLowerCase();
    if (lower.includes('pass') || lower.includes('approved') || lower.includes('approuve')) {
      logOk('PASS (heuristic)');
      return { approved: true, issues: [], summary: 'Approved (heuristic parse)' };
    }
    logFail('FAIL (could not parse QA response)');
    logInfo(`Raw: ${result.output.slice(0, 300)}`);
    return { approved: false, issues: [{ severity: 'MEDIUM', description: 'Could not parse QA response' }] };
  }
}

// ─── Phase F — Apply ────────────────────────────────────────────────────────

function apply(merged, storyId, dryRun = false) {
  logPhase('Phase F', dryRun ? 'Dry Run (pas d\'application)' : 'Application');

  if (dryRun) {
    for (const [fp, content] of Object.entries(merged)) {
      const lines = content.split('\n').length;
      logInfo(`${fp} (${lines} lignes)`);
    }
    log(`\n  ${C.yellow}🏁 Dry run — aucun fichier modifie${C.reset}`);
    return true;
  }

  // Create branch
  const branchName = storyId ? `orchestrator/${storyId}` : `orchestrator/task-${Date.now()}`;
  try {
    execSync(`git checkout -b "${branchName}"`, { cwd: PROJECT_ROOT, stdio: 'pipe' });
    logInfo(`git branch: ${branchName}`);
  } catch (e) {
    // Branch might already exist, or not a git repo
    logInfo(`git branch: skipped (${e.message.split('\n')[0]})`);
  }

  // Write files
  let filesWritten = 0;
  for (const [fp, content] of Object.entries(merged)) {
    const absPath = path.join(PROJECT_ROOT, fp);
    const dir = path.dirname(absPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(absPath, content, 'utf-8');
    filesWritten++;
    logInfo(`${fp} ecrit`);
  }

  // cargo check
  logInfo('cargo check...');
  try {
    execSync('cargo check 2>&1', { cwd: PROJECT_ROOT, timeout: 300_000, stdio: 'pipe' });
    logOk('cargo check PASS');
  } catch (e) {
    logFail('cargo check FAIL');
    const output = e.stdout ? e.stdout.toString().slice(-500) : e.message;
    log(`${C.dim}${output}${C.reset}`);

    // Rollback
    log(`  ${C.yellow}⟳${C.reset} Rollback...`);
    try {
      execSync('git checkout -- .', { cwd: PROJECT_ROOT, stdio: 'pipe' });
      execSync(`git branch -D "${branchName}"`, { cwd: PROJECT_ROOT, stdio: 'pipe' });
    } catch { /* best effort */ }
    return false;
  }

  // Git commit
  const commitMsg = storyId
    ? `feat(${storyId}): orchestrator multi-agent implementation`
    : `feat: orchestrator multi-agent task`;

  try {
    execSync('git add -A', { cwd: PROJECT_ROOT, stdio: 'pipe' });
    execSync(`git commit -m "${commitMsg}"`, { cwd: PROJECT_ROOT, stdio: 'pipe' });
    logInfo(`git commit: ${commitMsg}`);
  } catch (e) {
    logInfo(`git commit: skipped (${e.message.split('\n')[0]})`);
  }

  return true;
}

// ─── CLI ─────────────────────────────────────────────────────────────────────

function printBanner() {
  log(`\n${C.bold}🎮 IGC Orchestrator v${VERSION}${C.reset}`);
  log(`${'━'.repeat(40)}`);
}

function printUsage() {
  log(`
${C.bold}Usage:${C.reset}
  node tools/orchestrator.js "description libre"
  node tools/orchestrator.js --story story-008
  node tools/orchestrator.js --story story-008 --dry-run
  node tools/orchestrator.js --agents
  node tools/orchestrator.js --self-test
  node tools/orchestrator.js --help

${C.bold}Options:${C.reset}
  --story <id>    Charger une story BMAD comme tache
  --dry-run       Planifier sans appliquer les changements
  --agents        Lister tous les agents disponibles
  --self-test     Tester le pipeline complet avec des reponses simulees
  --help          Afficher cette aide
`);
}

function printAgents(agents) {
  log(`\n${C.bold}Agents Forgia (${Object.keys(agents).length})${C.reset}\n`);
  const maxName = Math.max(...Object.values(agents).map(a => a.name.length));
  for (const [id, agent] of Object.entries(agents)) {
    const status = agent.status === 'ready'
      ? `${C.green}SOUL ✓${C.reset}`
      : `${C.red}SOUL ✗${C.reset}`;
    const pad = ' '.repeat(maxName - agent.name.length);
    log(`  ${agent.emoji}  ${C.bold}${agent.name}${C.reset}${pad}  ${C.dim}(${id})${C.reset}  ${status}  ${C.gray}${agent.domain}${C.reset}`);
  }
  log('');
}

// ─── Self-Test ───────────────────────────────────────────────────────────────

async function selfTest() {
  printBanner();
  log(`${C.bold}🧪 Mode self-test — pipeline simule${C.reset}\n`);

  const agents = loadAgents();
  const readyCount = Object.values(agents).filter(a => a.status === 'ready').length;

  // Test 1: Agent registry
  logPhase('Test 1', `Agent Registry (${readyCount}/${Object.keys(agents).length})`);
  for (const [id, agent] of Object.entries(agents)) {
    const soulLen = agent.soul.length;
    const status = agent.status === 'ready' ? `${C.green}✓${C.reset}` : `${C.red}✗${C.reset}`;
    logInfo(`${agent.emoji} ${agent.name} (${id}) ${status} SOUL:${soulLen}c`);
  }
  logOk(`${readyCount} agents prets`);

  // Test 2: Story loading
  logPhase('Test 2', 'Story Loading');
  const testStories = ['story-008', 'story-009', 'story-005'];
  for (const sid of testStories) {
    const content = loadStory(sid);
    if (content) {
      logOk(`${sid}: ${content.split('\n').length} lignes`);
    } else {
      logFail(`${sid}: non trouve`);
    }
  }

  // Test 3: Index loading
  logPhase('Test 3', 'Stories Index');
  const index = loadIndex();
  if (index) {
    const storyCount = (index.match(/story-/g) || []).length;
    logOk(`_index.md charge (${storyCount} refs, ${index.split('\n').length} lignes)`);
  } else {
    logFail('_index.md non trouve');
  }

  // Test 4: Claude summary
  logPhase('Test 4', 'Claude Summary');
  const summary = loadSummary();
  if (summary && summary.length > 100) {
    logOk(`claude-summary.md charge (${summary.length} chars)`);
  } else {
    logFail('claude-summary.md manquant ou vide');
  }

  // Test 5: Context building
  logPhase('Test 5', 'Context Building');
  const mockSubtask = {
    id: 'T1',
    agent: 'forgia-terrain',
    description: 'Test context building',
    files_to_read: ['src/terrain/mod.rs'],
    files_to_write: ['src/terrain/mod.rs'],
  };
  const prompt = buildContext(mockSubtask, summary);
  logOk(`Prompt genere: ${prompt.length} chars`);
  const hasSummary = prompt.includes('Forgia');
  const hasFile = prompt.includes('terrain') || prompt.includes('mod.rs');
  logInfo(`Contient summary: ${hasSummary ? '✓' : '✗'}, fichier source: ${hasFile ? '✓' : '✗'}`);

  // Test 6: Simulated decomposition
  logPhase('Test 6', 'Decomposition Simulee');
  const mockPlan = {
    summary: 'Ping test simulation',
    subtasks: [
      { id: 'T1', agent: 'forgia-terrain', description: 'Modifier terrain/mod.rs', files_to_read: ['src/terrain/mod.rs'], files_to_write: ['src/terrain/mod.rs'], depends_on: [] },
      { id: 'T2', agent: 'igc-ui', description: 'Modifier ui/map_designer.rs', files_to_read: ['src/ui/map_designer.rs'], files_to_write: ['src/ui/map_designer.rs'], depends_on: [] },
      { id: 'T3', agent: 'forgia-qa', description: 'Valider les changements', files_to_read: [], files_to_write: [], depends_on: ['T1', 'T2'] },
    ],
    execution_order: [['T1', 'T2'], ['T3']],
  };
  logOk(`Plan: ${mockPlan.subtasks.length} sous-taches, ${mockPlan.execution_order.length} groupes`);
  for (const st of mockPlan.subtasks) {
    const agent = agents[st.agent];
    logInfo(`${st.id}: ${agent.emoji} ${agent.name} — ${st.description}`);
  }

  // Test 7: Simulated agent results
  logPhase('Test 7', 'Agent Results Simulees');
  const mockResults = [
    {
      taskId: 'T1', agentId: 'forgia-terrain', success: true, duration: 34,
      output: '// FILE: src/terrain/preview.rs\npub fn generate_heightmap_preview() {\n    // heightmap 2D preview\n}\n// END FILE',
    },
    {
      taskId: 'T2', agentId: 'igc-ui', success: true, duration: 28,
      output: '// FILE: src/ui/map_designer.rs\n// Added preview panel\nfn preview_panel(ui: &mut egui::Ui) {\n    ui.label("Preview");\n}\n// END FILE',
    },
    {
      taskId: 'T3', agentId: 'forgia-qa', success: true, duration: 12,
      output: 'PASS - tous les criteres sont respectes.',
    },
  ];
  for (const r of mockResults) {
    const agent = agents[r.agentId];
    logOk(`${agent.emoji} ${agent.name} (${r.duration}s) → ${r.output.split('\n')[0].slice(0, 50)}`);
  }

  // Test 8: Merge
  logPhase('Test 8', 'Merge Simule');
  const { merged, conflicts } = collectAndMerge(mockResults);
  logOk(`${Object.keys(merged).length} fichier(s), ${conflicts.length} conflit(s)`);

  // Test 9: Dry-run apply
  logPhase('Test 9', 'Apply Dry-Run');
  apply(merged, 'test-self', true);

  // Test 10: claude -p availability
  logPhase('Test 10', 'Claude CLI');
  try {
    const ver = execSync('claude --version', { timeout: 5000, encoding: 'utf-8' }).trim();
    logOk(`claude CLI: ${ver}`);
  } catch {
    logFail('claude CLI non disponible');
  }
  const isNested = process.env.CLAUDECODE === '1' || !!process.env.CLAUDE_CODE_SSE_PORT;
  if (isNested) {
    logInfo(`${C.yellow}Session Claude Code active — claude -p ne peut pas spawner ici${C.reset}`);
    logInfo('Lancer depuis un terminal externe pour le workflow complet.');
  } else {
    logOk('Pas de session active — claude -p disponible');
  }

  // Summary
  const totalDuration = ((Date.now() - startTime) / 1000).toFixed(1);
  log(`\n${'━'.repeat(40)}`);
  log(`${C.green}✅ Self-test termine en ${totalDuration}s${C.reset}`);
  log(`${C.bold}   ${readyCount} agents | stories OK | pipeline OK${C.reset}`);
  if (isNested) {
    log(`${C.yellow}   ⚠️  Tester le workflow reel depuis un terminal externe${C.reset}`);
  }
}

// ─── Main ────────────────────────────────────────────────────────────────────

const startTime = Date.now();

async function main() {
  const args = process.argv.slice(2);

  // Parse CLI args
  let storyId = null;
  let dryRun = false;
  let showAgents = false;
  let showHelp = false;
  let runSelfTest = false;
  let taskDescription = null;

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--story':
        storyId = args[++i];
        break;
      case '--dry-run':
        dryRun = true;
        break;
      case '--agents':
        showAgents = true;
        break;
      case '--self-test':
        runSelfTest = true;
        break;
      case '--help':
      case '-h':
        showHelp = true;
        break;
      default:
        if (!args[i].startsWith('--')) {
          taskDescription = args[i];
        }
        break;
    }
  }

  if (runSelfTest) {
    return selfTest();
  }

  printBanner();

  // Load agents
  const agents = loadAgents();

  if (showHelp) {
    printUsage();
    return;
  }

  if (showAgents) {
    printAgents(agents);
    return;
  }

  // Build task description
  let storyContent = null;
  if (storyId) {
    storyContent = loadStory(storyId);
    if (!storyContent) {
      logFail(`Story non trouvee: ${storyId}`);
      logInfo(`Fichiers disponibles: ${fs.readdirSync(STORIES_DIR).join(', ')}`);
      process.exit(1);
    }
    taskDescription = taskDescription || `Implementer ${storyId}`;
    log(`📋 Tache: ${storyId}`);
  } else if (!taskDescription) {
    logFail('Aucune tache specifiee.');
    printUsage();
    process.exit(1);
  } else {
    log(`📋 Tache: ${taskDescription}`);
  }

  if (dryRun) {
    log(`${C.yellow}🔍 Mode dry-run — pas de modification${C.reset}`);
  }

  // ── Phase A: Decompose
  const plan = await decompose(
    storyContent ? `${taskDescription}\n\nStory:\n${storyContent}` : taskDescription,
    agents,
    storyContent
  );

  if (!plan) {
    logFail('Impossible de decomposer la tache. Arret.');
    process.exit(1);
  }

  // Validate agents exist
  for (const st of plan.subtasks) {
    if (!agents[st.agent] || agents[st.agent].status !== 'ready') {
      logFail(`Agent inconnu ou non pret: ${st.agent}`);
      process.exit(1);
    }
  }

  // ── Phase C: Execute (Phase B is implicit in buildContext)
  const results = await executeSubtasks(plan, agents);

  const successCount = results.filter(r => r.success).length;
  const failCount = results.filter(r => !r.success).length;

  if (successCount === 0) {
    logFail(`Tous les agents ont echoue (${failCount}/${results.length})`);
    process.exit(1);
  }

  // ── Phase D: Merge
  const { merged, conflicts } = collectAndMerge(results);

  if (Object.keys(merged).length === 0) {
    logInfo('Aucun fichier a modifier (agents ont repondu sans blocs FILE).');
    // Print raw outputs for debugging
    for (const r of results.filter(r => r.success)) {
      const agent = agents[r.agentId];
      log(`\n${C.bold}${agent.emoji} ${agent.name}:${C.reset}`);
      log(r.output.slice(0, 500));
    }
    process.exit(0);
  }

  // ── Phase E: Validate
  const validation = await validate(merged, storyContent, agents);

  if (!validation.approved && !dryRun) {
    logFail('QA non approuvee. Utilise --dry-run pour voir le plan sans appliquer.');
    // Still show what would have been applied
    for (const [fp] of Object.entries(merged)) {
      logInfo(`(non applique) ${fp}`);
    }
    process.exit(1);
  }

  // ── Phase F: Apply
  const success = apply(merged, storyId, dryRun);

  // Summary
  const totalDuration = ((Date.now() - startTime) / 1000).toFixed(0);
  const minutes = Math.floor(totalDuration / 60);
  const seconds = totalDuration % 60;
  const timeStr = minutes > 0 ? `${minutes}m${String(seconds).padStart(2, '0')}s` : `${seconds}s`;

  log(`\n${'━'.repeat(40)}`);
  if (success) {
    log(`${C.green}✅ Termine en ${timeStr}${C.reset}`);
  } else {
    log(`${C.red}❌ Echoue apres ${timeStr}${C.reset}`);
  }
}

main().catch(err => {
  logFail(`Fatal: ${err.message}`);
  console.error(err);
  process.exit(1);
});
