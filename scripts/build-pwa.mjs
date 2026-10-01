import { readdir, readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
const root = 'dist/client'
async function files(dir) {
  return (
    await Promise.all(
      (await readdir(dir, { withFileTypes: true })).map(async (e) =>
        e.isDirectory() ? files(`${dir}/${e.name}`) : `${dir}/${e.name}`,
      ),
    )
  ).flat()
}
const assets = (await files(root))
  .filter((path) => !path.endsWith('/sw.js') && !path.endsWith('.map'))
  .map((path) => path.slice(root.length))
const version = createHash('sha256')
  .update(
    (await Promise.all(assets.map((path) => readFile(root + path))))
      .map((b) => createHash('sha256').update(b).digest('hex'))
      .join(''),
  )
  .digest('hex')
  .slice(0, 16)
await writeFile(
  `${root}/sw.js`,
  `const CACHE='littlewardrobe-shell-${version}';const ASSETS=${JSON.stringify(assets)};
self.addEventListener('install',event=>event.waitUntil(caches.open(CACHE).then(cache=>cache.addAll(ASSETS))));
self.addEventListener('activate',event=>event.waitUntil(caches.keys().then(keys=>Promise.all(keys.filter(key=>key.startsWith('littlewardrobe-shell-')&&key!==CACHE).map(key=>caches.delete(key)))).then(()=>self.clients.claim())));
self.addEventListener('message',event=>{if(event.data?.type==='ACTIVATE_UPDATE')self.skipWaiting()});
self.addEventListener('fetch',event=>{const url=new URL(event.request.url);if(url.origin!==self.location.origin||url.pathname.startsWith('/api/')||event.request.method!=='GET')return;
 if(event.request.mode==='navigate')event.respondWith(fetch(event.request).catch(()=>caches.open(CACHE).then(cache=>cache.match('/_shell.html'))));
 else if(ASSETS.includes(url.pathname))event.respondWith(caches.open(CACHE).then(cache=>cache.match(url.pathname)).then(hit=>hit||fetch(event.request)));
});\n`,
)
console.log(
  `Installable offline shell: ${assets.length} bundled assets, ${version}`,
)
