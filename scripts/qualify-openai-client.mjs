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

if (['1', 'true', 'yes'].includes((process.env.TALVIK_QUALIFY_TOOLS ?? '').toLowerCase())) {
  const tools = [{
    type: 'function',
    function: {
      name: 'get_weather',
      description: 'Get current weather for a city',
      parameters: {
        type: 'object',
        properties: { city: { type: 'string' } },
        required: ['city'],
      },
    },
  }]
  const toolRequest = await client.chat.completions.create({
    model,
    messages: [{
      role: 'user',
      content: 'Call get_weather with city Manila. Do not answer with plain text.',
    }],
    tools,
    tool_choice: 'auto',
    parallel_tool_calls: false,
    max_tokens: 16,
    context_size: 512,
    temperature: 0,
  })
  const toolCalls = toolRequest.choices[0]?.message.tool_calls ?? []
  const call = toolCalls[0]
  if (!call || toolCalls.length !== 1 || call.function.name !== 'get_weather') {
    throw new Error('OpenAI tool request did not return get_weather')
  }
  const required = await client.chat.completions.create({
    model,
    messages: [{
      role: 'user',
      content: 'Call get_weather with city set to Manila. Do not answer in prose.',
    }],
    tools,
    tool_choice: 'required',
    max_tokens: 16,
    context_size: 512,
    temperature: 0,
  })
  if (!required.choices[0]?.message.tool_calls?.length) {
    throw new Error('OpenAI required tool choice returned no tool call')
  }
  console.log('PASS required tool choice')

  const selected = await client.chat.completions.create({
    model,
    messages: [{
      role: 'user',
      content: 'Call get_weather with city set to Manila. Do not answer in prose.',
    }],
    tools,
    tool_choice: { type: 'function', function: { name: 'get_weather' } },
    max_tokens: 16,
    context_size: 512,
    temperature: 0,
  })
  const selectedCalls = selected.choices[0]?.message.tool_calls ?? []
  if (!selectedCalls.length || selectedCalls.some((tool) => tool.function.name !== 'get_weather')) {
    throw new Error('OpenAI function-specific tool choice returned the wrong tool')
  }
  console.log('PASS function-specific tool choice')

  const roundTrip = await client.chat.completions.create({
    model,
    messages: [
      { role: 'user', content: 'What is the weather in Manila?' },
      {
        role: 'assistant',
        content: null,
        tool_calls: [{
          id: call.id,
          type: 'function',
          function: {
            name: call.function.name,
            arguments: call.function.arguments,
          },
        }],
      },
      {
        role: 'tool',
        tool_call_id: call.id,
        name: 'get_weather',
        content: 'Clear, 29 degrees Celsius.',
      },
      { role: 'user', content: 'Summarize the tool result briefly.' },
    ],
    tools,
    max_tokens: 8,
    context_size: 512,
    temperature: 0,
  })
  if (!roundTrip.choices[0]?.message.content) {
    throw new Error('OpenAI tool-result round trip returned no assistant content')
  }
  console.log('PASS tool call and result message transport')
}

console.log('OPENAI_JAVASCRIPT_CLIENT_ACCEPTED')
