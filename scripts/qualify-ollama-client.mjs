#!/usr/bin/env node

import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(
  process.env.TALVIK_NODE_PREFIX
    ? path.join(process.env.TALVIK_NODE_PREFIX, 'talvik-qualifier.cjs')
    : import.meta.url,
)

let Ollama
try {
  Ollama = require('ollama').Ollama
} catch (error) {
  throw new Error(
    'Ollama JavaScript client is required; install it with `npm install --prefix <temp-dir> ollama@0.6.4` and set TALVIK_NODE_PREFIX',
    { cause: error },
  )
}

const baseUrl = (process.env.TALVIK_OLLAMA_BASE_URL ?? 'http://127.0.0.1:18080').replace(/\/+$/, '')
const model = process.env.TALVIK_MODEL
const token = process.env.TALVIK_API_KEY

if (!model || !token) {
  throw new Error('TALVIK_MODEL and TALVIK_API_KEY are required')
}

const headers = { Authorization: `Bearer ${token}` }
const client = new Ollama({ host: baseUrl, headers })
const options = { num_ctx: 512, num_predict: 4, temperature: 0 }

const version = await client.version()
if (!version.version) throw new Error('/api/version did not return a version')

const listed = (await client.list()).models.some((entry) => entry.model === model)
if (!listed) throw new Error(`model ${JSON.stringify(model)} was not returned by /api/tags`)
await client.ps()
await client.show({ model })
console.log('PASS version, tags, ps, and show')

const generated = await client.generate({
  model,
  prompt: 'Reply with a short greeting.',
  stream: false,
  options,
})
if (!generated.done || !generated.response) {
  throw new Error('non-streaming /api/generate returned no completed text')
}
console.log('PASS non-streaming generate')

let generatedText = ''
let generateDone = false
for await (const chunk of await client.generate({
  model,
  prompt: 'Reply with a short greeting.',
  stream: true,
  options,
})) {
  generatedText += chunk.response ?? ''
  generateDone = chunk.done
}
if (!hasCompletedText(generatedText, generateDone)) {
  throw new Error('streaming /api/generate did not return text and a terminal record')
}
console.log('PASS streaming generate')

const chat = await client.chat({
  model,
  messages: [{ role: 'user', content: 'Reply with a short greeting.' }],
  stream: false,
  options,
})
if (!chat.done || !chat.message.content) {
  throw new Error('non-streaming /api/chat returned no completed assistant message')
}
console.log('PASS non-streaming chat')

let chatText = ''
let chatDone = false
for await (const chunk of await client.chat({
  model,
  messages: [{ role: 'user', content: 'Reply with a short greeting.' }],
  stream: true,
  options,
})) {
  chatText += chunk.message.content ?? ''
  chatDone = chunk.done
}
if (!hasCompletedText(chatText, chatDone)) {
  throw new Error('streaming /api/chat did not return text and a terminal record')
}
console.log('PASS streaming chat')

const missing = await fetch(`${baseUrl}/api/generate`, {
  method: 'POST',
  headers: { ...headers, 'Content-Type': 'application/json' },
  body: JSON.stringify({
    model: `${model}-talvik-qualification-missing`,
    prompt: 'This request must fail.',
    stream: false,
  }),
})
const missingBody = await missing.json()
if (missing.status !== 404 || typeof missingBody.error !== 'string') {
  throw new Error('unknown model did not return an Ollama-shaped 404 error')
}
console.log('PASS unknown-model error')

console.log('OLLAMA_JAVASCRIPT_CLIENT_ACCEPTED')

function hasCompletedText(text, done) {
  return text.length > 0 && done
}
