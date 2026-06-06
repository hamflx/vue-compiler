<template>
  <header class="base-layout-header" :data-collapsed="collapsed ? 'yes' : 'no'">
    <AppLogo class="base-layout-header__logo" :compact="collapsed" />
    <nav class="base-layout-header__nav" aria-label="Primary">
      <a
        v-for="section in primarySections"
        :key="section.id"
        class="base-layout-header__nav-link"
        :class="{ 'is-active': section.id === activeSection }"
        :href="section.href"
      >
        <span class="base-layout-header__nav-label">{{ section.label }}</span>
        <span v-if="section.badge" class="base-layout-header__nav-badge">
          {{ formatCount(section.badge) }}
        </span>
      </a>
    </nav>
    <HeaderSearch
      class="base-layout-header__search"
      v-model="query"
      :placeholder="searchPlaceholder"
      :suggestions="searchSuggestions"
      @select="handleSearchSelect"
    />
    <div class="base-layout-header__actions">
      <button
        v-for="action in visibleActions"
        :key="action.id"
        class="base-layout-header__action"
        :aria-label="action.label"
        :disabled="action.disabled"
        @click="action.run"
      >
        <span class="base-layout-header__action-icon">{{ action.icon }}</span>
        <span v-if="!collapsed" class="base-layout-header__action-label">{{ action.label }}</span>
      </button>
      <NotificationBell
        :items="notifications"
        :count="notificationCount"
        @open="emit('open-notifications')"
      />
      <UserMenu
        :user="currentUser"
        :teams="teams"
        :compact="collapsed"
        @logout="emit('logout')"
      />
    </div>
    <section class="base-layout-header__status" aria-live="polite">
      <p v-if="flags.showTrial" class="base-layout-header__trial">
        {{ trialMessage }}
      </p>
      <p v-else-if="flags.showMaintenance" class="base-layout-header__maintenance">
        {{ maintenanceMessage }}
      </p>
      <p v-else class="base-layout-header__ready">
        {{ readyMessage }}
      </p>
    </section>
  </header>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import AppLogo from './AppLogo.vue'
import HeaderSearch from './HeaderSearch.vue'
import NotificationBell from './NotificationBell.vue'
import UserMenu from './UserMenu.vue'
import { formatCount, primarySections } from '../fixtures/navigation'
import { useAccountSummary, useFeatureFlags } from '../composables/header'
import type { HeaderAction, HeaderNotification, HeaderTeam, UserSummary } from '../types/header'

defineOptions({ name: 'BaseLayoutHeader' })

interface Props {
  collapsed?: boolean
  activeSection: string
  currentUser: UserSummary
  teams: HeaderTeam[]
  notifications: HeaderNotification[]
  actions: HeaderAction[]
}

const props = withDefaults(defineProps<Props>(), {
  collapsed: false,
})

const emit = defineEmits<{
  logout: []
  'open-notifications': []
  'select-search': [value: string]
}>()

const flags = useFeatureFlags()
const account = useAccountSummary(() => props.currentUser.id)
const query = ref('')

const notificationCount = computed(() => props.notifications.filter((item) => !item.read).length)
const visibleActions = computed(() => props.actions.filter((action) => !action.hidden))
const searchPlaceholder = computed(() => `Search ${account.value.workspaceName}`)
const searchSuggestions = computed(() => primarySections.map((section) => section.label))
const trialMessage = computed(() => `${account.value.daysLeft} days left in trial`)
const maintenanceMessage = computed(() => 'Maintenance window scheduled')
const readyMessage = computed(() => `${props.currentUser.name} is signed in`)

function handleSearchSelect(value: string) {
  query.value = value
  emit('select-search', value)
}
</script>

<style scoped>
.base-layout-header {
  align-items: center;
  display: grid;
  gap: 12px;
  grid-template-columns: auto minmax(0, 1fr) minmax(220px, 320px) auto;
}
</style>
