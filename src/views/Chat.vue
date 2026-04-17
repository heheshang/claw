<script setup lang="ts">
import { ref, onMounted, nextTick, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { useLogger } from '../composables/useLogger'

const { initLogger, info, warn } = useLogger()

interface Message {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: Date
  toolCalls?: ToolCall[]
  isStreaming?: boolean
}

interface ToolCall {
  id: string
  name: string
  input: string
  output?: string
  is_error?: boolean
  status: 'pending' | 'executing' | 'success' | 'error'
  result?: string
  error?: string
}

interface SessionInfo {
  session_id: string
  message_count: number
  model: string
}

interface SessionHistoryItem {
  id: string
  created_at_ms: number
  updated_at_ms: number
  message_count: number
  first_message_preview: string | null
}

interface LoadedMessage {
  role: string
  text: string
}

interface ToolDefinition {
  name: string
  description?: string
  input_schema: any
}

interface UploadedFile {
  name: string
  content: string
  type: string
  size: number
}

interface FileReference {
  name: string
  path: string
  content: string
}

interface ToolCategory {
  name: string
  icon: string
  tools: string[]
}

interface FileInfo {
  path: string
  name: string
  is_dir: boolean
  size: number
}

const toolCategories: ToolCategory[] = [
  { name: '文件操作', icon: '📁', tools: ['read', 'write', 'edit'] },
  { name: '搜索', icon: '🔍', tools: ['glob', 'grep'] },
  { name: '执行', icon: '💻', tools: ['bash', 'lspath'] },
  { name: 'Web', icon: '🌐', tools: ['web_search', 'web_fetch'] },
  { name: 'Git', icon: '📊', tools: ['git_status', 'git_diff', 'git_log', 'git_branch'] },
  { name: '任务', icon: '✅', tools: ['todo_create', 'todo_list', 'todo_update', 'todo_delete'] },
  { name: 'Notebook', icon: '📓', tools: ['notebook_read', 'notebook_edit'] },
  { name: '高级', icon: '🚀', tools: ['subagent'] }
]

const slashCommands = [
  '/help', '/model', '/session', '/compact', '/clear', '/status', '/cost', '/stats',
  '/tasks', '/review', '/security-review', '/diff', '/commit', '/pr', '/init',
  '/sandbox', '/agents', '/mcp', '/skills', '/system-prompt'
]

const messages = ref<Message[]>([])
const inputMessage = ref('')
const isLoading = ref(false)
const messagesContainer = ref<HTMLElement | null>(null)
const sessionInfo = ref<SessionInfo | null>(null)
const currentPermission = ref('danger')
const showHistory = ref(false)
const sessionHistory = ref<SessionHistoryItem[]>([])
const historyLoading = ref(false)
const showToolsPanel = ref(false)
const availableTools = ref<ToolDefinition[]>([])
const inputRef = ref<HTMLInputElement | null>(null)
const streamingContent = ref('')
const currentStreamingMessageId = ref<string | null>(null)

// File upload
const uploadedFiles = ref<UploadedFile[]>([])
const isDragging = ref(false)
const fileInputRef = ref<HTMLInputElement | null>(null)
const MAX_FILE_SIZE = 10 * 1024 * 1024 // 10MB

// File reference (@ mention)
const showFilePicker = ref(false)
const fileSearchQuery = ref('')
const fileSearchResults = ref<FileInfo[]>([])
const fileReferences = ref<FileReference[]>([])
// const filePickerPosition = ref({ top: 0, left: 0 }) // Reserved for future positioning
const fileSearchInputRef = ref<HTMLInputElement | null>(null)
const selectedFileIndex = ref(0)
const atTriggerPos = ref(0)
const isLoadingFiles = ref(false)

// Multiline input (reserved for future implementation)
// const multilineMode = ref(false)
// const multilineContent = ref('')

const permissionLevels = [
  { value: 'readonly', label: '只读', tools: ['read', 'glob', 'grep', 'lspath'] },
  { value: 'workspacewrite', label: '可写', tools: ['read', 'write', 'edit', 'glob', 'grep', 'lspath', 'git_status', 'git_log', 'git_branch'] },
  { value: 'danger', label: '完全访问', tools: ['read', 'write', 'edit', 'glob', 'grep', 'bash', 'lspath', 'web_search', 'web_fetch', 'git_status', 'git_diff', 'git_log', 'git_branch', 'todo_create', 'todo_list', 'todo_update', 'todo_delete', 'notebook_read', 'notebook_edit', 'subagent'] }
]

const currentTools = computed(() => {
  const level = permissionLevels.find(p => p.value === currentPermission.value)
  return level?.tools || []
})

const canSend = computed(() => (inputMessage.value.trim() || uploadedFiles.value.length > 0) && !isLoading.value)

const availableToolCategories = computed(() => {
  return toolCategories.filter(cat =>
    cat.tools.some(tool => currentTools.value.includes(tool))
  )
})

// Tab completion
const showCompletion = ref(false)
const completions = ref<string[]>([])
const selectedCompletionIndex = ref(0)

const inputHistory = ref<string[]>([])
const historyIndex = ref(-1)

onMounted(async () => {
  await initLogger()
  info('Chat page loaded')

  try {
    const tools = await invoke<ToolDefinition[]>('harness_get_tools')
    availableTools.value = tools
    info('Loaded tools', { count: tools.length })
  } catch (e) {
    warn('Failed to load tools', { error: e })
  }

  const savedPermission = localStorage.getItem('permission')
  if (savedPermission && permissionLevels.some(p => p.value === savedPermission)) {
    currentPermission.value = savedPermission
    try {
      await invoke<string>('harness_set_permission', { permission: savedPermission })
    } catch (e) {
      warn('Failed to restore permission', { error: e })
    }
  }

  try {
    sessionInfo.value = await invoke<SessionInfo>('harness_get_session')
    info('Session loaded', { sessionId: sessionInfo.value?.session_id })
  } catch (e) {
    warn('Failed to load session', { error: e })
  }

  messages.value.push({
    id: 'welcome',
    role: 'assistant',
    content: `👋 欢迎使用 AI Agent！

这是一个智能编程助手，可以帮助你：

📁 **文件操作** - read, write, edit
🔍 **搜索工具** - glob, grep
💻 **执行工具** - bash, lspath
🌐 **Web 工具** - web_search, web_fetch
📊 **Git 操作** - git_status, git_diff, git_log, git_branch
✅ **任务管理** - todo_create, todo_list, todo_update, todo_delete
📓 **Notebook** - notebook_read, notebook_edit
🚀 **高级** - subagent

输入你的问题开始吧！也可以点击工具按钮快速插入命令。`,
    timestamp: new Date()
  })
})

onUnmounted(() => {
  // Cleanup
})

async function sendMessage() {
  if (!canSend.value) return

  const userMessage = inputMessage.value.trim()
  if (!userMessage && uploadedFiles.value.length === 0 && fileReferences.value.length === 0) return

  // Build message content including uploaded files and file references
  let fullMessage = userMessage

  // Add file references
  if (fileReferences.value.length > 0) {
    const refContents = fileReferences.value.map(f => {
      return `\`\`\`${f.path}\n${f.content}\n\`\`\``
    }).join('\n\n')
    fullMessage = fullMessage
      ? `${userMessage}\n\n引用文件：\n\n${refContents}`
      : `请分析以下引用文件：\n\n${refContents}`
  }

  // Add uploaded files
  if (uploadedFiles.value.length > 0) {
    const fileContents = uploadedFiles.value.map(f => {
      return `\`\`\`${f.name}\n${f.content}\n\`\`\``
    }).join('\n\n')
    fullMessage = fullMessage
      ? `${fullMessage}\n\n上传的文件：\n\n${fileContents}`
      : `请分析以下文件内容：\n\n${fileContents}`
  }

  // Clear file references
  fileReferences.value = []

  // Add to history
  if (userMessage) {
    inputHistory.value.push(userMessage)
    historyIndex.value = -1
  }

  inputMessage.value = ''
  showCompletion.value = false

  // Clear uploaded files after adding to message
  uploadedFiles.value = []

  messages.value.push({
    id: `user-${Date.now()}`,
    role: 'user',
    content: fullMessage,
    timestamp: new Date()
  })

  isLoading.value = true
  scrollToBottom()

  // Create streaming message
  const assistantMsgId = `assistant-${Date.now()}`
  streamingContent.value = ''
  currentStreamingMessageId.value = assistantMsgId

  // Initialize message with empty tool calls
  messages.value.push({
    id: assistantMsgId,
    role: 'assistant',
    content: '',
    timestamp: new Date(),
    isStreaming: true,
    toolCalls: []
  })

  // Track accumulated content and tool calls
  let accumulatedText = ''
  let toolCalls: ToolCall[] = []

  // Set up event listeners for streaming
  let unlistenChunk: UnlistenFn | null = null
  let unlistenDone: UnlistenFn | null = null
  let unlistenError: UnlistenFn | null = null
  let unlistenStart: UnlistenFn | null = null

  try {
    info('Sending message (streaming)', { message: userMessage.substring(0, 50) })

    // Listen for stream chunks
    unlistenChunk = await listen<{
      event_type: string
      content: string
      tool_id?: string
      tool_name?: string
      tool_input?: string
      tool_output?: string
      is_error?: boolean
    }>('stream_chunk', (event) => {
      const data = event.payload
      console.log('[Stream] Received chunk:', data.event_type, data.content?.substring(0, 50))

      if (data.event_type === 'text_delta') {
        // Accumulate text
        accumulatedText += data.content
        streamingContent.value = accumulatedText

        // Update message content in real-time
        const msgIndex = messages.value.findIndex(m => m.id === assistantMsgId)
        if (msgIndex !== -1) {
          messages.value[msgIndex].content = accumulatedText
        }
        scrollToBottom()
      } else if (data.event_type === 'tool_use') {
        // Add tool call to the list
        const toolCall: ToolCall = {
          id: data.tool_id || '',
          name: data.tool_name || '',
          input: data.tool_input || '',
          status: 'executing',
          result: ''
        }
        toolCalls.push(toolCall)

        // Update message tool calls
        const msgIndex = messages.value.findIndex(m => m.id === assistantMsgId)
        if (msgIndex !== -1) {
          messages.value[msgIndex].toolCalls = [...toolCalls]
        }
        scrollToBottom()
      } else if (data.event_type === 'tool_result') {
        // Update the last tool call with result
        if (toolCalls.length > 0) {
          const lastIndex = toolCalls.length - 1
          toolCalls[lastIndex].status = data.is_error ? 'error' : 'success'
          toolCalls[lastIndex].output = data.tool_output
          toolCalls[lastIndex].result = data.tool_output || ''

          // Update message tool calls
          const msgIndex = messages.value.findIndex(m => m.id === assistantMsgId)
          if (msgIndex !== -1) {
            messages.value[msgIndex].toolCalls = [...toolCalls]
          }
          scrollToBottom()
        }
      }
    })

    // Listen for stream done
    unlistenDone = await listen('stream_done', () => {
      // Streaming complete
      currentStreamingMessageId.value = null

      const msgIndex = messages.value.findIndex(m => m.id === assistantMsgId)
      if (msgIndex !== -1) {
        messages.value[msgIndex].isStreaming = false
      }

      isLoading.value = false
      scrollToBottom()
    })

    // Listen for stream errors
    unlistenError = await listen<{ error: string }>('stream_error', (event) => {
      const msgIndex = messages.value.findIndex(m => m.id === assistantMsgId)
      if (msgIndex !== -1) {
        messages.value[msgIndex].content = `❌ 错误: ${event.payload.error}`
        messages.value[msgIndex].isStreaming = false
      }
      currentStreamingMessageId.value = null
      isLoading.value = false
      scrollToBottom()
    })

    // Listen for stream start
    unlistenStart = await listen<{ message_id: string }>('stream_start', (event) => {
      console.log('[Stream] Received stream_start:', event.payload)
      info('Stream started')
    })

    // Call the streaming endpoint
    await invoke('harness_send_message_stream', {
      request: { message: userMessage }
    })

    // Refresh session info
    sessionInfo.value = await invoke<SessionInfo>('harness_get_session')
    info('Response received', { textLength: accumulatedText.length, toolCalls: toolCalls.length })
  } catch (e: any) {
    warn('Harness error', { error: e.toString() })

    streamingContent.value = ''
    currentStreamingMessageId.value = null

    const msgIndex = messages.value.findIndex(m => m.id === assistantMsgId)
    if (msgIndex !== -1) {
      messages.value[msgIndex].content = `❌ 错误: ${e.toString()}`
      messages.value[msgIndex].isStreaming = false
    }

    isLoading.value = false
    scrollToBottom()
  } finally {
    // Cleanup event listeners
    if (unlistenChunk) unlistenChunk()
    if (unlistenDone) unlistenDone()
    if (unlistenError) unlistenError()
    if (unlistenStart) unlistenStart()
  }
}

function handleKeydown(e: KeyboardEvent) {
  // Handle file picker navigation first
  if (showFilePicker.value) {
    handleFilePickerKeydown(e)
    return
  }

  // Tab completion
  if (e.key === 'Tab' && showCompletion.value && completions.value.length > 0) {
    e.preventDefault()
    const completion = completions.value[selectedCompletionIndex.value]
    if (completion) {
      if (completion.startsWith('/')) {
        inputMessage.value = completion + ' '
      } else {
        inputMessage.value = completion + '('
      }
      showCompletion.value = false
    }
    return
  }

  // Navigate completions
  if (showCompletion.value && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
    e.preventDefault()
    if (e.key === 'ArrowDown') {
      selectedCompletionIndex.value = (selectedCompletionIndex.value + 1) % completions.value.length
    } else {
      selectedCompletionIndex.value = (selectedCompletionIndex.value - 1 + completions.value.length) % completions.value.length
    }
    return
  }

  // Close completion on escape
  if (e.key === 'Escape') {
    showCompletion.value = false
    return
  }

  // History navigation
  if (e.key === 'ArrowUp' && inputHistory.value.length > 0) {
    if (historyIndex.value === -1) {
      historyIndex.value = inputHistory.value.length - 1
    } else if (historyIndex.value > 0) {
      historyIndex.value--
    }
    inputMessage.value = inputHistory.value[historyIndex.value]
    e.preventDefault()
    return
  }

  if (e.key === 'ArrowDown') {
    if (historyIndex.value !== -1 && historyIndex.value < inputHistory.value.length - 1) {
      historyIndex.value++
      inputMessage.value = inputHistory.value[historyIndex.value]
    } else {
      historyIndex.value = -1
      inputMessage.value = ''
    }
    e.preventDefault()
    return
  }

  // Cancel on Ctrl+C when there's input
  if (e.key === 'c' && e.ctrlKey && inputMessage.value.trim()) {
    e.preventDefault()
    inputMessage.value = ''
    showCompletion.value = false
    return
  }
}

function handleInput() {
  const input = inputMessage.value

  // Check for @ file reference
  const atIndex = input.lastIndexOf('@')
  if (atIndex !== -1 && (atIndex === 0 || /\s/.test(input[atIndex - 1]))) {
    // Check if there's no space after @ or if @ is at start/in a new "word"
    const afterAt = input.slice(atIndex + 1)
    if (!afterAt.includes(' ') && !showFilePicker.value) {
      // Trigger file picker
      fileSearchQuery.value = afterAt
      atTriggerPos.value = atIndex
      openFilePicker()
      searchFiles(afterAt)
      return
    }
  }

  if (showFilePicker.value && !input.slice(atTriggerPos.value).startsWith('@')) {
    showFilePicker.value = false
    fileSearchQuery.value = ''
  }

  // Check for slash command or tool completion
  if (input.startsWith('/')) {
    const prefix = input.toLowerCase()
    const matches = slashCommands.filter(cmd => cmd.startsWith(prefix))
    if (matches.length > 0) {
      completions.value = matches
      showCompletion.value = true
      selectedCompletionIndex.value = 0
    } else {
      showCompletion.value = false
    }
  } else if (input.includes('(')) {
    // Tool name completion
    const parts = input.split('(')
    const prefix = parts[0].trim()
    const toolMatches = currentTools.value.filter(t => t.startsWith(prefix))
    if (toolMatches.length > 0) {
      completions.value = toolMatches
      showCompletion.value = true
      selectedCompletionIndex.value = 0
    } else {
      showCompletion.value = false
    }
  } else {
    showCompletion.value = false
  }
}

// File reference functions
async function searchFiles(query: string) {
  if (!query) {
    fileSearchResults.value = []
    return
  }
  try {
    const results = await invoke<FileInfo[]>('search_files', {
      pattern: query,
      workspaceRoot: '.'
    })
    fileSearchResults.value = results
  } catch (e) {
    warn('File search failed', { error: e })
    fileSearchResults.value = []
  }
}

function openFilePicker() {
  showFilePicker.value = true
  fileSearchQuery.value = ''
  fileSearchResults.value = []
  selectedFileIndex.value = 0
  // Reset input to remove partial @query
  inputMessage.value = inputMessage.value.slice(0, atTriggerPos.value)
  nextTick(() => {
    fileSearchInputRef.value?.focus()
  })
}

async function insertFileReference(file: FileInfo) {
  // For folders, recursively read all code files
  if (file.is_dir) {
    isLoadingFiles.value = true
    showFilePicker.value = false

    try {
      // Get all code files in the directory
      const files = await invoke<FileInfo[]>('harness_read_directory', {
        path: file.path,
        extensions: ['ts', 'tsx', 'js', 'jsx', 'vue', 'rs', 'py', 'go', 'java', 'cpp', 'c', 'h', 'css', 'scss', 'json', 'yaml', 'yml', 'toml', 'md', 'txt']
      })

      // Read content of each file
      for (const f of files) {
        try {
          const content = await invoke<string>('harness_read_file', { path: f.path })
          fileReferences.value.push({
            name: f.name,
            path: f.path,
            content: content
          })
        } catch (e) {
          warn('Failed to read file', { path: f.path, error: e })
        }
      }

      info('Loaded files from directory', { count: files.length })
    } catch (e) {
      warn('Failed to read directory', { error: e })
    } finally {
      isLoadingFiles.value = false
    }

    // Clear the @query from input
    const beforeAt = inputMessage.value.slice(0, atTriggerPos.value)
    inputMessage.value = beforeAt
    inputRef.value?.focus()
    return
  }

  try {
    // Read file content
    const content = await invoke<string>('harness_read_file', { path: file.path })

    // Add to file references
    fileReferences.value.push({
      name: file.name,
      path: file.path,
      content: content
    })

    // Replace @query with @filename in input
    const beforeAt = inputMessage.value.slice(0, atTriggerPos.value)
    const afterQuery = inputMessage.value.slice(atTriggerPos.value + fileSearchQuery.value.length + 1)
    inputMessage.value = beforeAt + '@' + file.name + afterQuery

    // Close file picker
    showFilePicker.value = false
    fileSearchQuery.value = ''
    fileSearchResults.value = []
  } catch (e) {
    warn('Failed to read file', { error: e })
  }
}

function handleFilePickerKeydown(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    selectedFileIndex.value = Math.min(selectedFileIndex.value + 1, fileSearchResults.value.length - 1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    selectedFileIndex.value = Math.max(selectedFileIndex.value - 1, 0)
  } else if (e.key === 'Enter') {
    e.preventDefault()
    if (fileSearchResults.value[selectedFileIndex.value]) {
      insertFileReference(fileSearchResults.value[selectedFileIndex.value])
    }
  } else if (e.key === 'Escape') {
    showFilePicker.value = false
    fileSearchQuery.value = ''
  }
}

function insertToolCommand(toolName: string) {
  inputMessage.value = toolName + '('
  inputRef.value?.focus()
}

async function resetSession() {
  try {
    await invoke('harness_reset_session')
    messages.value = [{
      id: 'welcome',
      role: 'assistant',
      content: '✨ 会话已重置！开始新对话吧。',
      timestamp: new Date()
    }]
    sessionInfo.value = await invoke<SessionInfo>('harness_get_session')
    info('Session reset')
  } catch (e) {
    warn('Failed to reset session', { error: e })
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (messagesContainer.value) {
      messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
    }
  })
}

function formatTime(date: Date): string {
  return date.toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit'
  })
}

async function changePermission() {
  try {
    await invoke<string>('harness_set_permission', {
      permission: currentPermission.value
    })
    localStorage.setItem('permission', currentPermission.value)
    info('Permission changed', { permission: currentPermission.value })
  } catch (e) {
    warn('Failed to change permission', { error: e })
  }
}

async function toggleHistory() {
  showHistory.value = !showHistory.value
  if (showHistory.value) {
    await loadSessionHistory()
  }
}

function toggleToolsPanel() {
  showToolsPanel.value = !showToolsPanel.value
}

async function loadSessionHistory() {
  historyLoading.value = true
  try {
    const sessions = await invoke<SessionHistoryItem[]>('harness_get_session_list')
    sessionHistory.value = sessions
  } catch (e) {
    warn('Failed to load session history', { error: e })
  } finally {
    historyLoading.value = false
  }
}

async function loadSession(sessionId: string) {
  try {
    const session = await invoke<{ id: string; message_count: number }>('harness_load_session', { sessionId })
    info('Session loaded', { sessionId })

    const loadedMessages = await invoke<LoadedMessage[]>('harness_get_session_messages', { sessionId })

    messages.value = loadedMessages.map((m, i) => ({
      id: `${sessionId}-${i}`,
      role: m.role as 'user' | 'assistant',
      content: m.text,
      timestamp: new Date()
    }))

    sessionInfo.value = {
      session_id: session.id,
      message_count: session.message_count,
      model: 'claude-sonnet-4-20250514'
    }

    showHistory.value = false
  } catch (e) {
    warn('Failed to load session', { error: e })
  }
}

async function createNewSession() {
  try {
    const session = await invoke<{ id: string }>('harness_create_session')
    info('New session created', { sessionId: session.id })
    messages.value = []
    sessionInfo.value = {
      session_id: session.id,
      message_count: 0,
      model: 'claude-sonnet-4-20250514'
    }
    showHistory.value = false
  } catch (e) {
    warn('Failed to create session', { error: e })
  }
}

function formatHistoryTime(timestamp: number): string {
  const date = new Date(timestamp)
  return date.toLocaleDateString('zh-CN', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  })
}

const toolDescriptions: Record<string, string> = {
  read: '读取文件内容 (path, offset?, limit?)',
  write: '写入文件内容 (path, content)',
  edit: '编辑文件内容 (path, old_string, new_string)',
  glob: '按模式搜索文件 (pattern, path?)',
  grep: '在文件中搜索文本 (pattern, path?, case_insensitive?)',
  bash: '执行Shell命令 (command, timeout_secs?)',
  lspath: '列出目录内容 (path?)',
  web_search: '网络搜索 (query)',
  web_fetch: '获取URL内容 (url)',
  git_status: '显示Git状态 (path?)',
  git_diff: '显示Git差异 (path?, commit?, file?)',
  git_log: '显示Git提交日志 (path?, max_count?, format?)',
  git_branch: '管理Git分支 (path?, list?, branch_name?, delete?)',
  todo_create: '创建任务 (content, priority?)',
  todo_list: '列出任务 (status?)',
  todo_update: '更新任务 (id, content?, status?)',
  todo_delete: '删除任务 (id)',
  notebook_read: '读取Jupyter笔记本 (path)',
  notebook_edit: '编辑Jupyter笔记本 (path, cell_index?, source?, cell_type?)',
  subagent: '执行子任务 (task, prompt)'
}

function getToolDescription(toolName: string): string {
  const toolDef = availableTools.value.find(t => t.name === toolName)
  if (toolDef?.description) {
    return toolDef.description
  }
  return toolDescriptions[toolName] || toolName
}

function getToolStatusIcon(status: string): string {
  switch (status) {
    case 'pending': return '⏳'
    case 'executing': return '⚡'
    case 'success': return '✅'
    case 'error': return '❌'
    default: return '❓'
  }
}

// File upload handlers
function triggerFileUpload() {
  fileInputRef.value?.click()
}

function handleFileInput(event: Event) {
  const input = event.target as HTMLInputElement
  if (input.files) {
    handleFiles(Array.from(input.files))
  }
  input.value = '' // Reset input
}

function handleFiles(files: File[]) {
  for (const file of files) {
    if (file.size > MAX_FILE_SIZE) {
      warn(`File too large: ${file.name} (max ${MAX_FILE_SIZE / 1024 / 1024}MB)`)
      continue
    }
    readFileContent(file).then(content => {
      uploadedFiles.value.push({
        name: file.name,
        content,
        type: file.type,
        size: file.size
      })
    })
  }
}

async function readFileContent(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = reader.result as string
      resolve(result)
    }
    reader.onerror = () => reject(reader.error)
    reader.readAsText(file)
  })
}

function removeFile(index: number) {
  uploadedFiles.value.splice(index, 1)
}

function handleDragOver(e: DragEvent) {
  e.preventDefault()
  isDragging.value = true
}

function handleDragLeave(e: DragEvent) {
  e.preventDefault()
  isDragging.value = false
}

function handleDrop(e: DragEvent) {
  e.preventDefault()
  isDragging.value = false
  if (e.dataTransfer?.files) {
    handleFiles(Array.from(e.dataTransfer.files))
  }
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / 1024 / 1024).toFixed(1) + ' MB'
}
</script>

<template>
  <div class="chat-container">
    <header class="chat-header">
      <div class="header-left">
        <h1>AI 助手</h1>
        <span v-if="sessionInfo" class="session-badge">
          {{ sessionInfo.session_id.substring(0, 8) }}
        </span>
        <span v-if="sessionInfo" class="model-badge" :title="sessionInfo.model">
          {{ sessionInfo.model.split('-')[1] || sessionInfo.model }}
        </span>
      </div>
      <div class="header-right">
        <button class="icon-btn" @click="toggleToolsPanel" :class="{ active: showToolsPanel }" title="工具面板 (T)">
          🛠️
        </button>
        <button class="icon-btn" @click="toggleHistory" title="历史记录 (H)">
          📜
        </button>
        <select v-model="currentPermission" @change="changePermission" class="permission-select">
          <option v-for="level in permissionLevels" :key="level.value" :value="level.value">
            {{ level.label }}
          </option>
        </select>
        <button class="icon-btn" @click="resetSession" title="重置会话">
          🔄
        </button>
      </div>
    </header>

    <!-- Session History Sidebar -->
    <div v-if="showHistory" class="history-sidebar">
      <div class="sidebar-header">
        <h3>聊天历史</h3>
        <button class="new-session-btn" @click="createNewSession">+ 新对话</button>
      </div>
      <div v-if="historyLoading" class="sidebar-loading">
        <div class="spinner-small"></div>
      </div>
      <div v-else class="sidebar-list">
        <div
          v-for="session in sessionHistory"
          :key="session.id"
          class="sidebar-item"
          @click="loadSession(session.id)"
        >
          <div class="item-preview">
            {{ session.first_message_preview || '新对话' }}
          </div>
          <div class="item-meta">
            <span>{{ formatHistoryTime(session.updated_at_ms) }}</span>
            <span>{{ session.message_count }} 条</span>
          </div>
        </div>
        <div v-if="sessionHistory.length === 0" class="empty-state">
          暂无历史记录
        </div>
      </div>
    </div>

    <!-- Tools Panel Sidebar -->
    <div v-if="showToolsPanel" class="tools-sidebar">
      <div class="sidebar-header">
        <h3>工具面板</h3>
        <span class="sidebar-hint">点击插入工具</span>
      </div>
      <div class="sidebar-content">
        <div v-for="category in availableToolCategories" :key="category.name" class="tool-category">
          <div class="category-header">
            <span class="category-icon">{{ category.icon }}</span>
            <span class="category-name">{{ category.name }}</span>
          </div>
          <div class="category-tools">
            <button
              v-for="tool in category.tools"
              :key="tool"
              class="tool-btn"
              @click="insertToolCommand(tool)"
              :title="getToolDescription(tool)"
            >
              {{ tool }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Messages -->
    <main class="chat-main" ref="messagesContainer">
      <div class="messages-list">
        <div
          v-for="message in messages"
          :key="message.id"
          :class="['message', message.role, { streaming: message.isStreaming }]"
        >
          <div class="message-avatar">
            <span v-if="message.role === 'user'">👤</span>
            <span v-else-if="message.role === 'assistant'">🤖</span>
            <span v-else>⚙️</span>
          </div>
          <div class="message-content">
            <div class="message-bubble" v-html="formatContent(message.content)"></div>

            <!-- Tool Calls -->
            <div v-if="message.toolCalls?.length" class="tool-calls">
              <div class="tool-calls-header">
                <span class="tool-count">{{ message.toolCalls.length }} 个工具调用</span>
              </div>
              <div
                v-for="tool in message.toolCalls"
                :key="tool.id"
                :class="['tool-call', tool.status]"
              >
                <div class="tool-call-header">
                  <span class="tool-status-icon">{{ getToolStatusIcon(tool.status) }}</span>
                  <span class="tool-name">{{ tool.name }}</span>
                </div>
                <code class="tool-input">{{ formatJson(tool.input) }}</code>
                <div v-if="tool.result" class="tool-result" :class="{ error: tool.is_error }">
                  {{ tool.result }}
                </div>
              </div>
            </div>

            <!-- Streaming indicator -->
            <div v-if="message.isStreaming" class="streaming-indicator">
              <span class="dot"></span>
              <span class="dot"></span>
              <span class="dot"></span>
            </div>

            <div class="message-time">{{ formatTime(message.timestamp) }}</div>
          </div>
        </div>
      </div>
    </main>

    <!-- Completion Popup -->
    <div v-if="showCompletion && completions.length > 0" class="completion-popup">
      <div
        v-for="(completion, index) in completions"
        :key="completion"
        :class="['completion-item', { selected: index === selectedCompletionIndex }]"
        @click="() => {
          if (completion.startsWith('/')) {
            inputMessage = completion + ' '
          } else {
            inputMessage = completion + '('
          }
          showCompletion = false
        }"
      >
        {{ completion }}
      </div>
    </div>

    <!-- File Picker Popup -->
    <div v-if="showFilePicker" class="file-picker-popup">
      <div class="file-picker-header">
        <span class="file-picker-icon">📄</span>
        <span>引用文件</span>
      </div>
      <div class="file-search-input-wrapper">
        <input
          ref="fileSearchInputRef"
          v-model="fileSearchQuery"
          type="text"
          placeholder="搜索文件..."
          class="file-search-input"
          @input="searchFiles(fileSearchQuery)"
          @keydown="handleFilePickerKeydown"
          autofocus
        />
      </div>
      <div class="file-search-results">
        <div
          v-for="(file, index) in fileSearchResults"
          :key="file.path"
          :class="['file-search-item', { selected: index === selectedFileIndex }]"
          @click="insertFileReference(file)"
          @mouseenter="selectedFileIndex = index"
        >
          <span class="file-item-icon">{{ file.is_dir ? '📁' : '📄' }}</span>
          <span class="file-item-name">{{ file.name }}</span>
          <span class="file-item-path">{{ file.path }}</span>
        </div>
        <div v-if="fileSearchResults.length === 0" class="file-search-empty">
          {{ fileSearchQuery ? '未找到文件' : '输入文件名搜索' }}
        </div>
      </div>
    </div>

    <!-- Drag overlay -->
    <div
      v-if="isDragging"
      class="drag-overlay"
      @dragover="handleDragOver"
      @dragleave="handleDragLeave"
      @drop="handleDrop"
    >
      <div class="drag-content">
        <span class="drag-icon">📄</span>
        <span class="drag-text">拖放文件到此处上传</span>
      </div>
    </div>

    <!-- Footer -->
    <footer class="chat-footer">
      <!-- Uploaded files -->
      <div v-if="uploadedFiles.length > 0" class="uploaded-files">
        <div
          v-for="(file, index) in uploadedFiles"
          :key="index"
          class="file-tag"
        >
          <span class="file-icon">📄</span>
          <span class="file-name" :title="file.name">{{ file.name }}</span>
          <span class="file-size">{{ formatFileSize(file.size) }}</span>
          <button class="file-remove" @click="removeFile(index)">×</button>
        </div>
      </div>

      <div class="input-wrapper">
        <!-- Hidden file input -->
        <input
          ref="fileInputRef"
          type="file"
          multiple
          class="hidden-file-input"
          @change="handleFileInput"
        />

        <!-- Upload button -->
        <button
          class="upload-btn"
          @click="triggerFileUpload"
          title="上传文件"
        >
          📎
        </button>

        <input
          ref="inputRef"
          v-model="inputMessage"
          type="text"
          placeholder="输入消息或 /命令 ..."
          :disabled="isLoading"
          @keydown="handleKeydown"
          @input="handleInput"
          @keyup.enter="sendMessage"
          class="message-input"
        />
        <button
          class="send-btn"
          :disabled="!canSend"
          @click="sendMessage"
          :class="{ loading: isLoading }"
        >
          <span v-if="!isLoading">➤</span>
          <span v-else class="loading-spinner"></span>
        </button>
      </div>
      <div class="tools-hint">
        <span class="hint-item">Tab: 自动补全</span>
        <span class="hint-item">↑↓: 历史记录</span>
        <span class="hint-item">Ctrl+C: 取消</span>
        <span class="hint-item">拖拽上传文件</span>
      </div>
    </footer>
  </div>
</template>

<script lang="ts">
function formatContent(content: string): string {
  if (!content) return ''

  let formatted = content
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')

  // Code blocks with syntax highlighting
  formatted = formatted.replace(/```(\w*)\n?([\s\S]*?)```/g, (_, lang, code) => {
    return `<pre class="code-block"><code class="language-${lang || 'plain'}">${code.trim()}</code></pre>`
  })

  // Inline code
  formatted = formatted.replace(/`([^`]+)`/g, '<code>$1</code>')

  // Bold
  formatted = formatted.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')

  // Italic
  formatted = formatted.replace(/\*([^*]+)\*/g, '<em>$1</em>')

  // Links
  formatted = formatted.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>')

  // Line breaks
  formatted = formatted.replace(/\n/g, '<br>')

  return formatted
}

function formatJson(json: string): string {
  try {
    const obj = JSON.parse(json)
    return JSON.stringify(obj, null, 2)
  } catch {
    return json
  }
}
</script>

<style scoped>
.chat-container {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: #0f0f1a;
  position: relative;
}

.chat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.chat-header h1 {
  color: #fff;
  font-size: 16px;
  font-weight: 600;
  margin: 0;
}

.session-badge {
  padding: 3px 8px;
  background: rgba(102, 126, 234, 0.2);
  border: 1px solid rgba(102, 126, 234, 0.3);
  border-radius: 10px;
  font-size: 10px;
  color: rgba(255, 255, 255, 0.7);
  font-family: monospace;
}

.model-badge {
  padding: 3px 8px;
  background: rgba(118, 75, 162, 0.3);
  border: 1px solid rgba(118, 75, 162, 0.4);
  border-radius: 10px;
  font-size: 10px;
  color: rgba(255, 255, 255, 0.6);
  font-family: monospace;
  max-width: 80px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.permission-select {
  padding: 6px 10px;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 6px;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
  outline: none;
}

.permission-select:focus {
  border-color: #667eea;
}

.permission-select option {
  background: #1a1a2e;
  color: #fff;
}

.icon-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  font-size: 14px;
  transition: all 0.2s;
}

.icon-btn:hover {
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
}

.icon-btn.active {
  background: rgba(102, 126, 234, 0.3);
  border-color: rgba(102, 126, 234, 0.5);
  color: #fff;
}

/* Sidebars */
.history-sidebar,
.tools-sidebar {
  position: absolute;
  top: 0;
  left: 0;
  width: 280px;
  height: 100%;
  background: rgba(20, 20, 35, 0.98);
  border-right: 1px solid rgba(255, 255, 255, 0.1);
  display: flex;
  flex-direction: column;
  z-index: 100;
  animation: slideIn 0.2s ease;
}

@keyframes slideIn {
  from { transform: translateX(-100%); }
  to { transform: translateX(0); }
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.sidebar-header h3 {
  color: #fff;
  font-size: 14px;
  font-weight: 600;
  margin: 0;
}

.sidebar-hint {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
}

.new-session-btn {
  padding: 5px 10px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.2s;
}

.new-session-btn:hover {
  opacity: 0.9;
  transform: scale(1.02);
}

.sidebar-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px;
}

.spinner-small {
  width: 20px;
  height: 20px;
  border: 2px solid rgba(255, 255, 255, 0.1);
  border-top-color: #667eea;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.sidebar-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.sidebar-item {
  padding: 12px 14px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  margin-bottom: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.sidebar-item:hover {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(102, 126, 234, 0.3);
}

.item-preview {
  color: #fff;
  font-size: 13px;
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-meta {
  display: flex;
  justify-content: space-between;
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
}

.empty-state {
  text-align: center;
  color: rgba(255, 255, 255, 0.4);
  padding: 40px;
  font-size: 13px;
}

/* Tools Sidebar */
.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.tool-category {
  margin-bottom: 16px;
}

.category-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
  padding-bottom: 4px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.category-icon {
  font-size: 12px;
}

.category-name {
  color: rgba(255, 255, 255, 0.7);
  font-size: 11px;
  font-weight: 600;
}

.category-tools {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.tool-btn {
  padding: 4px 8px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 10px;
  font-family: monospace;
  cursor: pointer;
  transition: all 0.2s;
}

.tool-btn:hover {
  background: rgba(102, 126, 234, 0.2);
  border-color: rgba(102, 126, 234, 0.4);
  color: #fff;
}

/* Messages */
.chat-main {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
}

.messages-list {
  max-width: 800px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.message {
  display: flex;
  gap: 12px;
  animation: fadeIn 0.3s ease;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

.message.user {
  flex-direction: row-reverse;
}

.message-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  flex-shrink: 0;
}

.message.user .message-avatar {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.message.assistant .message-avatar {
  background: rgba(255, 255, 255, 0.1);
}

.message-content {
  max-width: 78%;
}

.message-bubble {
  padding: 12px 16px;
  border-radius: 14px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  color: #fff;
  font-size: 14px;
}

.message.user .message-bubble {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border-bottom-right-radius: 4px;
}

.message.assistant .message-bubble {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-bottom-left-radius: 4px;
}

.message.streaming .message-bubble {
  border-color: rgba(102, 126, 234, 0.3);
}

.message-bubble :deep(code) {
  background: rgba(0, 0, 0, 0.4);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: 'Fira Code', 'Consolas', monospace;
  font-size: 0.9em;
}

.message-bubble :deep(pre.code-block) {
  background: rgba(0, 0, 0, 0.5);
  padding: 12px;
  border-radius: 8px;
  overflow-x: auto;
  margin: 8px 0;
}

.message-bubble :deep(pre.code-block code) {
  background: none;
  padding: 0;
  display: block;
  white-space: pre;
}

.message-bubble :deep(a) {
  color: #667eea;
  text-decoration: none;
}

.message-bubble :deep(a:hover) {
  text-decoration: underline;
}

/* Tool Calls */
.tool-calls {
  margin-top: 10px;
  padding: 10px;
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
}

.tool-calls-header {
  margin-bottom: 8px;
}

.tool-count {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
}

.tool-call {
  padding: 8px 10px;
  margin-bottom: 6px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 6px;
}

.tool-call:last-child {
  margin-bottom: 0;
}

.tool-call.pending {
  border-color: rgba(255, 193, 7, 0.3);
}

.tool-call.executing {
  border-color: rgba(102, 126, 234, 0.5);
  animation: pulse 1s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}

.tool-call.success {
  border-color: rgba(40, 167, 69, 0.3);
}

.tool-call.error {
  border-color: rgba(220, 53, 69, 0.3);
}

.tool-call-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.tool-status-icon {
  font-size: 12px;
}

.tool-name {
  color: #667eea;
  font-weight: 600;
  font-size: 12px;
  font-family: monospace;
}

.tool-input {
  display: block;
  color: rgba(255, 255, 255, 0.6);
  font-size: 11px;
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 100px;
  overflow-y: auto;
}

.tool-result {
  margin-top: 6px;
  padding: 6px 8px;
  background: rgba(40, 167, 69, 0.1);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 11px;
  white-space: pre-wrap;
  max-height: 150px;
  overflow-y: auto;
}

.tool-result.error {
  background: rgba(220, 53, 69, 0.1);
  color: #ff6b6b;
}

/* Streaming indicator */
.streaming-indicator {
  display: inline-flex;
  gap: 4px;
  margin-left: 8px;
  vertical-align: middle;
}

.streaming-indicator .dot {
  width: 6px;
  height: 6px;
  background: rgba(102, 126, 234, 0.7);
  border-radius: 50%;
  animation: bounce 1.4s infinite ease-in-out;
}

.streaming-indicator .dot:nth-child(1) { animation-delay: -0.32s; }
.streaming-indicator .dot:nth-child(2) { animation-delay: -0.16s; }

@keyframes bounce {
  0%, 80%, 100% { transform: scale(0); }
  40% { transform: scale(1); }
}

.message-time {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.3);
  margin-top: 4px;
  padding: 0 4px;
}

.message.user .message-time {
  text-align: right;
}

/* Completion Popup */
.completion-popup {
  position: fixed;
  bottom: 90px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(30, 30, 50, 0.98);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  overflow: hidden;
  z-index: 200;
  min-width: 200px;
  max-width: 400px;
}

.completion-item {
  padding: 8px 14px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 13px;
  font-family: monospace;
  cursor: pointer;
  transition: background 0.15s;
}

.completion-item:hover,
.completion-item.selected {
  background: rgba(102, 126, 234, 0.3);
  color: #fff;
}

/* File Picker Popup */
.file-picker-popup {
  position: fixed;
  bottom: 90px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(30, 30, 50, 0.98);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 12px;
  overflow: hidden;
  z-index: 200;
  min-width: 300px;
  max-width: 500px;
  max-height: 350px;
  display: flex;
  flex-direction: column;
}

.file-picker-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: rgba(102, 126, 234, 0.15);
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  font-size: 13px;
  color: rgba(255, 255, 255, 0.8);
}

.file-picker-icon {
  font-size: 14px;
}

.file-search-input-wrapper {
  padding: 8px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.file-search-input {
  width: 100%;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 6px;
  color: #fff;
  font-size: 13px;
  outline: none;
}

.file-search-input:focus {
  border-color: #667eea;
}

.file-search-results {
  flex: 1;
  overflow-y: auto;
  max-height: 250px;
}

.file-search-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
  cursor: pointer;
  transition: background 0.15s;
}

.file-search-item:hover,
.file-search-item.selected {
  background: rgba(102, 126, 234, 0.2);
}

.file-item-icon {
  font-size: 14px;
  flex-shrink: 0;
}

.file-item-name {
  color: #fff;
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.file-item-path {
  color: rgba(255, 255, 255, 0.4);
  font-size: 11px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-left: auto;
}

.file-search-empty {
  padding: 20px;
  text-align: center;
  color: rgba(255, 255, 255, 0.4);
  font-size: 13px;
}

/* Footer */
.chat-footer {
  padding: 12px 20px;
  background: rgba(255, 255, 255, 0.02);
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.input-wrapper {
  display: flex;
  gap: 10px;
  max-width: 800px;
  margin: 0 auto;
}

.message-input {
  flex: 1;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 24px;
  color: #fff;
  font-size: 14px;
  outline: none;
  transition: all 0.2s;
}

.message-input::placeholder {
  color: rgba(255, 255, 255, 0.4);
}

.message-input:focus {
  border-color: #667eea;
  background: rgba(255, 255, 255, 0.1);
}

.message-input:disabled {
  opacity: 0.5;
}

.send-btn {
  width: 44px;
  height: 44px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  border-radius: 50%;
  font-size: 16px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.send-btn:hover:not(:disabled) {
  transform: scale(1.05);
}

.send-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.send-btn.loading {
  pointer-events: none;
}

.loading-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

/* Upload button */
.upload-btn {
  width: 44px;
  height: 44px;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 50%;
  font-size: 16px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.upload-btn:hover {
  background: rgba(255, 255, 255, 0.15);
  transform: scale(1.05);
}

.hidden-file-input {
  display: none;
}

/* Uploaded files */
.uploaded-files {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 10px;
  max-width: 800px;
  margin-left: auto;
  margin-right: auto;
}

.file-tag {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: rgba(102, 126, 234, 0.15);
  border: 1px solid rgba(102, 126, 234, 0.3);
  border-radius: 16px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.8);
}

.file-icon {
  font-size: 12px;
}

.file-name {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-size {
  color: rgba(255, 255, 255, 0.4);
  font-size: 10px;
}

.file-remove {
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.5);
  cursor: pointer;
  font-size: 14px;
  padding: 0;
  margin-left: 2px;
  line-height: 1;
}

.file-remove:hover {
  color: rgba(255, 255, 255, 0.8);
}

/* Drag overlay */
.drag-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(15, 15, 26, 0.95);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  border: 3px dashed rgba(102, 126, 234, 0.5);
  border-radius: 12px;
  margin: 10px;
}

.drag-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  color: rgba(255, 255, 255, 0.7);
}

.drag-icon {
  font-size: 48px;
  opacity: 0.7;
}

.drag-text {
  font-size: 16px;
  font-weight: 500;
}

.tools-hint {
  display: flex;
  justify-content: center;
  gap: 16px;
  margin-top: 8px;
  max-width: 800px;
  margin-left: auto;
  margin-right: auto;
}

.hint-item {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.3);
}
</style>
