import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const pluginDir = fileURLToPath(new URL('../vendor/tauri-plugin-music-notification/', import.meta.url))
const patch = fileURLToPath(new URL('./patches/music-notification-start-service.patch', import.meta.url))

function gitApply(args) {
  const result = spawnSync('git', ['apply', ...args, patch], {
    cwd: pluginDir,
    encoding: 'utf8',
  })
  if (result.error) throw result.error
  return result
}

if (gitApply(['--reverse', '--check']).status !== 0) {
  const result = gitApply([])
  if (result.status !== 0 && gitApply(['--reverse', '--check']).status !== 0) {
    console.error(result.stderr.trim())
    process.exit(1)
  }
}
