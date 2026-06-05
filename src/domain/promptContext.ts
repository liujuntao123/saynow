interface VocabularyItem {
  term: string;
  alias?: string;
  enabled: boolean;
}

interface StylePrompt {
  name: string;
  prompt: string;
  enabled: boolean;
}

interface HistoryRecord {
  text: string;
  status: 'success' | 'failed' | 'processing';
}

export interface PromptContextInput {
  vocabulary: VocabularyItem[];
  styles: StylePrompt[];
  records: HistoryRecord[];
}

const FORMAT_EXAMPLE =
  '格式示例：将“五月二十号，也就是五天前，我五点半就起床了，在北京游览了三个景点。晚上，我还和国外的朋友聊了会儿天，非常happy”输出为“5月20号，也就是5天前，我5:30就起床了，在北京游览了3个景点。晚上，我还和国外的朋友聊了会儿天，非常happy”。';

export function buildPromptPreview(input: PromptContextInput): string {
  const vocabulary = input.vocabulary
    .filter((item) => item.enabled)
    .slice(0, 30)
    .map((item) => `- ${item.term}${item.alias ? ` (${item.alias})` : ''}`)
    .join('\n');

  const style = input.styles.find((item) => item.enabled)?.prompt ?? '输出自然、准确的简体中文文本。';

  const history = input.records
    .filter((record) => record.status === 'success' && record.text.trim())
    .slice(0, 5)
    .map((record) => `- ${record.text.trim()}`)
    .join('\n');

  return [
    '你是一个桌面端语音识别助手。只输出最终识别文本，不输出解释。',
    `输出风格：${style}`,
    FORMAT_EXAMPLE,
    vocabulary ? `用户词库：\n${vocabulary}` : '',
    history ? `相关历史：\n${history}` : '',
  ]
    .filter(Boolean)
    .join('\n\n');
}
