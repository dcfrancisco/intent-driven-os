#!/usr/bin/env node

import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(
  process.env.TALVIK_NODE_PREFIX
    ? path.join(process.env.TALVIK_NODE_PREFIX, 'talvik-qualifier.cjs')
    : import.meta.url,
)

let Ollama
let OpenAI
try {
  Ollama = require('ollama').Ollama
  const imported = require('openai')
  OpenAI = imported.default ?? imported.OpenAI
} catch (error) {
  throw new Error(
    'Ollama and OpenAI JavaScript clients are required; install ollama@0.6.4 and openai@7.45.1 outside the repository and set TALVIK_NODE_PREFIX',
    { cause: error },
  )
}

const model = process.env.TALVIK_EMBED_MODEL
const token = process.env.TALVIK_API_KEY
if (!model || !token) {
  throw new Error('TALVIK_EMBED_MODEL and TALVIK_API_KEY are required')
}

const ollamaUrl = (process.env.TALVIK_OLLAMA_BASE_URL ?? 'http://127.0.0.1:18080').replace(/\/+$/, '')
const ollama = new Ollama({
  host: ollamaUrl,
  headers: { Authorization: `Bearer ${token}` },
})
const ollamaEmbed = await ollama.embed({
  model,
  input: ['embedding qualification one', 'embedding qualification two'],
})
if (
  ollamaEmbed.embeddings.length !== 2 ||
  !ollamaEmbed.embeddings.every((vector) => vector.length > 0)
) {
  throw new Error('/api/embed did not return two nonempty vectors')
}
const legacy = await ollama.embeddings({
  model,
  prompt: 'legacy embedding qualification',
})
if (!legacy.embedding.length) throw new Error('/api/embeddings returned an empty vector')
console.log('PASS Ollama batch and legacy embeddings')

const openai = new OpenAI({
  baseURL: process.env.TALVIK_OPENAI_BASE_URL ?? 'http://127.0.0.1:18080/v1',
  apiKey: token,
})
const openaiEmbed = await openai.embeddings.create({
  model,
  input: ['embedding qualification one', 'embedding qualification two'],
})
if (
  openaiEmbed.data.length !== 2 ||
  !openaiEmbed.data.every((item) => item.embedding.length > 0)
) {
  throw new Error('/v1/embeddings did not return two nonempty vectors')
}
console.log('PASS OpenAI batch embeddings')
console.log('EMBEDDING_JAVASCRIPT_CLIENTS_ACCEPTED')
