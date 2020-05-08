const path = require('path')
const { promisify } = require('util')

let { getUnchangedEntries, cacheEntries } = require('../native/index.node')

cacheEntries = promisify(cacheEntries)

class HashCachePlugin {
  constructor ({ cacheDir } = {}) {
    // TODO: make this more generic
    const { CACHE_DIR, USE_HASH_CACHE = 1, NODE_ENV } = process.env
    this.cacheDIR = path.join((cacheDir || CACHE_DIR), 'hash-cache-plugin')
    if (USE_HASH_CACHE && (!this.cacheDIR)) {
      throw new Error('Must provide CACHE_DIR when using HashCache')
    }
    this.useHashCache = parseInt(USE_HASH_CACHE) === 1 && NODE_ENV !== 'production'
    this.apply = this.apply.bind(this)
  }

  static extractChunks (chunkGroups) {
    return chunkGroups.reduce(
      (memo, chunkGroup) =>
        memo.concat(
          chunkGroup.chunks,
          HashCachePlugin.extractChunks(chunkGroup.getChildren())
        ),
      []
    )
  }

  static statsEntryHelper (stats) {
    const entries = {}

    Array.from(stats.compilation.entrypoints.values()).forEach((entrypoint) => {
      const entry = entrypoint.name
      const chunks = HashCachePlugin.extractChunks([entrypoint])
      chunks.forEach((chunk) => {
        const hash = chunk.renderedHash
        Array.from(chunk.modulesIterable).forEach((module) => {
          if (!entries[entry]) {
            entries[entry] = {
              files: [],
              hash,
              name: entry
            }
          }
          if (
            module.sourceMap &&
             module.sourceMap.sources &&
             module.sourceMap.sources instanceof Array
          ) {
            entries[entry].files = entries[entry].files.concat(
              module.sourceMap.sources
            )
          }
          if (module.resource) {
            entries[entry].files.push(module.resource)
          }
        })
      })
    })
    return Object.values(entries)
  }

  apply (compiler) {
    compiler.hooks.beforeRun.tapPromise('FilterCacheEntries', async (compilerHook) => {
      const entries = compilerHook.options.entry
      if (this.useHashCache) {
        const unchangedEntries = getUnchangedEntries(this.cacheDIR)
        Object.keys(entries).forEach((name) => {
          if (unchangedEntries.indexOf(name) >= 0) {
            delete entries[name]
          }
        })
      }

      if (Object.keys(compilerHook.options.entry).length > 0) {
        console.log(`Building the following entries [${Object.keys(entries)}]`)
      } else {
        console.log('No entries required building...')
        process.exit(0)
      }
    })

    compiler.hooks.done.tapPromise('HashCachePlugin', async (stats) => {
      if (this.useHashCache) {
        return cacheEntries(this.cacheDIR, HashCachePlugin.statsEntryHelper(stats).map((entry) => {
          const { files = [], name } = entry
          return { files, name }
        }))
      }
    })
  }
}

module.exports = {
  HashCachePlugin
}
