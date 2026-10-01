import { execFileSync, spawn } from 'node:child_process'
import { createServer } from 'node:net'
import { randomUUID } from 'node:crypto'
const listener = createServer()
await new Promise((r) => listener.listen(0, '127.0.0.1', r))
const port = listener.address().port
await new Promise((r) => listener.close(r))
const name = `littlewardrobe-test-${randomUUID()}`
const origin = `http://127.0.0.1:${port}`
try {
  execFileSync(
    'docker',
    [
      'run',
      '-d',
      '--name',
      name,
      '--network',
      'host',
      '--read-only',
      '--tmpfs',
      '/data:uid=10001,gid=10001,mode=700',
      '--tmpfs',
      '/tmp:uid=10001,gid=10001,mode=1777',
      '-e',
      `LITTLEWARDROBE_BIND=127.0.0.1:${port}`,
      '-e',
      `LITTLEWARDROBE_ORIGIN=${origin}`,
      '-e',
      'LITTLEWARDROBE_SETUP_TOKEN=isolated-browser-bootstrap-token',
      'ghcr.io/fruitiex/littlewardrobe:main',
    ],
    { stdio: 'pipe' },
  )
  const child = spawn(process.execPath, ['scripts/verify-production.mjs'], {
    env: { ...process.env, LITTLEWARDROBE_BROWSER_URL: origin },
    stdio: 'inherit',
  })
  const code = await new Promise((r) => child.on('exit', r))
  if (code !== 0) throw new Error(`Container browser checks failed (${code})`)
  console.log(
    'The Nix container passed the complete production browser suite as an unprivileged process with a read-only root and temporary isolated data.',
  )
} finally {
  execFileSync('docker', ['rm', '-f', name], { stdio: 'pipe' })
}
