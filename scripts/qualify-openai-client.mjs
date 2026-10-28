#!/usr/bin/env node

import path from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(
  process.env.TALVIK_NODE_PREFIX
    ? path.join(process.env.TALVIK_NODE_PREFIX, 'talvik-qualifier.cjs')
    : import.meta.url,
)

let OpenAI
try {
  const imported = require('openai')
  OpenAI = imported.default ?? imported.OpenAI
} catch (error) {
  throw new Error(
    'OpenAI JavaScript client is required; install it with `npm install --prefix <temp-dir> openai@7.45.1` and set TALVIK_NODE_PREFIX',
    { cause: error },
  )
}

const baseURL = process.env.TALVIK_OPENAI_BASE_URL ?? 'http://127.0.0.1:18080/v1'
const model = process.env.TALVIK_MODEL
const apiKey = process.env.TALVIK_API_KEY

if (!model || !apiKey) {
  throw new Error('TALVIK_MODEL and TALVIK_API_KEY are required')
}

const client = new OpenAI({ baseURL, apiKey })
const models = await client.models.list()
if (!models.data.some((entry) => entry.id === model)) {
  throw new Error(`model ${JSON.stringify(model)} was not returned by /v1/models`)
}
console.log('PASS model discovery')

const plain = await client.chat.completions.create({
  model,
  messages: [{ role: 'user', content: 'Reply with a short greeting.' }],
  max_tokens: 4,
})
if (!plain.choices[0]?.message.content) {
  throw new Error('non-stream Chat Completions response had no assistant content')
}
console.log('PASS non-stream Chat Completions')

const stream = await client.chat.completions.create({
  model,
  messages: [{ role: 'user', content: 'Reply with a short greeting.' }],
  max_tokens: 4,
  stream: true,
})
let streamText = ''
for await (const chunk of stream) {
  streamText += chunk.choices[0]?.delta.content ?? ''
}
if (!streamText) throw new Error('streaming Chat Completions response was empty')
console.log('PASS streaming Chat Completions')

const legacy = await client.completions.create({
  model,
  prompt: 'Complete with a short greeting:',
  max_tokens: 4,
})
if (legacy.object !== 'text_completion' || !legacy.choices[0]?.text) {
  throw new Error('legacy Completions response did not use the text_completion shape')
}
console.log('PASS non-stream legacy Completions')

const legacyStream = await client.completions.create({
  model,
  prompt: 'Complete with one word:',
  max_tokens: 4,
  stream: true,
})
let legacyText = ''
for await (const chunk of legacyStream) {
  legacyText += chunk.choices[0]?.text ?? ''
}
if (!legacyText) throw new Error('streaming legacy Completions response was empty')
console.log('PASS streaming legacy Completions')

const response = await client.responses.create({
  model,
  instructions: 'Reply briefly.',
  input: 'Say hello.',
  max_output_tokens: 4,
})
if (response.object !== 'response' || !response.output_text) {
  throw new Error('Responses API did not return a text response')
}
console.log('PASS non-stream Responses')

console.log('OPENAI_JAVASCRIPT_CLIENT_ACCEPTED')
