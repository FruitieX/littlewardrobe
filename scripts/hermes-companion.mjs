// Launch inside Hermes's managed runtime, never inside this coding session's auth.
import assert from 'node:assert/strict'
import { execFileSync, spawn } from 'node:child_process'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
const args = process.argv.slice(2)
assert.equal(
  args.shift(),
  '--profile',
  'Usage: hermes-companion.mjs --profile NAME [--setup URL | --pair-stdin]',
)
const profile = args.shift()
assert.ok(
  profile && /^[a-z0-9][a-z0-9-]*$/.test(profile) && profile !== 'default',
  'Choose a dedicated personal profile',
)
assert.ok(
  args.length === 0 ||
    (args.length === 1 && args[0] === '--pair-stdin') ||
    (args.length === 2 && args[0] === '--setup'),
  'Invalid companion arguments',
)
const env = {
  ...process.env,
  HERMES_HOME: join(homedir(), '.hermes/profiles', profile),
}
const command = JSON.parse(
  execFileSync(
    process.env.LITTLEWARDROBE_HERMES_BINARY || 'hermes',
    ['--print-runtime-command', '--module', 'runpy'],
    { env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] },
  ),
)
const entry = "runpy.run_module('runpy', run_name='__main__', alter_sys=True)"
assert.ok(
  command[1] === '-I' && command[2] === '-c' && command[3].endsWith(entry),
  'Unrecognized Hermes runtime',
)
command[3] =
  command[3].slice(0, -entry.length) +
  "runpy.run_path(sys.argv.pop(1), run_name='__main__')"
command.push(
  fileURLToPath(new URL('./hermes-companion.py', import.meta.url)),
  ...args,
)
const child = spawn(command[0], command.slice(1), { env, stdio: 'inherit' })
let stopping = false
for (const signal of ['SIGTERM', 'SIGINT'])
  process.on(signal, () => {
    stopping = true
    child.kill(signal)
  })
process.exitCode = await new Promise((done) => {
  child.once('error', () => done(1))
  child.once('exit', (code) => done(stopping ? 0 : (code ?? 1)))
})
