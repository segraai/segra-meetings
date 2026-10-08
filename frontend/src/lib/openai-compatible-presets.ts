// Common OpenAI-compatible providers, used to pre-fill the custom endpoint form.
export interface OpenAICompatiblePreset {
  name: string;
  endpoint: string;
  /** Where to get an API key; omitted for local servers */
  keyUrl?: string;
}

export const OPENAI_COMPATIBLE_PRESETS: OpenAICompatiblePreset[] = [
  { name: 'Nous Research', endpoint: 'https://inference-api.nousresearch.com/v1', keyUrl: 'https://portal.nousresearch.com' },
  { name: 'Together AI', endpoint: 'https://api.together.xyz/v1', keyUrl: 'https://api.together.ai/settings/api-keys' },
  { name: 'Fireworks', endpoint: 'https://api.fireworks.ai/inference/v1', keyUrl: 'https://fireworks.ai/account/api-keys' },
  { name: 'DeepSeek', endpoint: 'https://api.deepseek.com/v1', keyUrl: 'https://platform.deepseek.com/api_keys' },
  { name: 'Mistral', endpoint: 'https://api.mistral.ai/v1', keyUrl: 'https://console.mistral.ai/api-keys' },
  { name: 'xAI', endpoint: 'https://api.x.ai/v1', keyUrl: 'https://console.x.ai' },
  { name: 'Google Gemini', endpoint: 'https://generativelanguage.googleapis.com/v1beta/openai', keyUrl: 'https://aistudio.google.com/apikey' },
  { name: 'Cerebras', endpoint: 'https://api.cerebras.ai/v1', keyUrl: 'https://cloud.cerebras.ai' },
  { name: 'LM Studio (local)', endpoint: 'http://localhost:1234/v1' },
  { name: 'vLLM (local)', endpoint: 'http://localhost:8000/v1' },
];
