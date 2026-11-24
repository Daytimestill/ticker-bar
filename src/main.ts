import { createApp } from 'vue'
import App from './App.vue'
import QuickView from './components/QuickView.vue'
import './styles.css'

const quickView = new URLSearchParams(window.location.search).get('view') === 'quick'
if (quickView) {
  document.documentElement.classList.add('is-quick-view')
  document.body.classList.add('is-quick-view')
}

createApp(quickView ? QuickView : App).mount('#app')
