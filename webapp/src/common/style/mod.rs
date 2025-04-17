use constcat::concat;

mod components;
mod home;
mod variables;

// Export new design system
pub use components::BASE_COMPONENTS;
pub use home::HOME_STYLES;
pub use variables::CSS_VARIABLES;

// Modern style bundling
pub const MODERN_STYLES: &str = concat!(
    r#"
/* Global resets and base styles */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  color: var(--text-primary);
  background-color: var(--background);
  line-height: 1.5;
}

a {
  color: var(--primary);
  text-decoration: none;
}

a:hover {
  text-decoration: underline;
}

/* Combine our design system parts */"#,
    CSS_VARIABLES,
    BASE_COMPONENTS,
    r#"
/* Application-specific styles */
.app-header {
  background-color: var(--surface);
  box-shadow: var(--shadow-sm);
  position: sticky;
  top: 0;
  z-index: 10;
}

.nav-container {
  display: flex;
  height: var(--header-height);
  align-items: center;
  justify-content: space-between;
  padding: 0 var(--space-4);
}

.nav-links {
  display: flex;
  gap: var(--space-4);
}

.nav-link {
  color: var(--text-secondary);
  font-weight: 500;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  transition: color var(--transition-fast) var(--easing-standard),
              background-color var(--transition-fast) var(--easing-standard);
}

.nav-link:hover {
  color: var(--text-primary);
  background-color: var(--neutral-100);
  text-decoration: none;
}

.nav-link.active {
  color: var(--primary);
  background-color: rgba(59, 130, 246, 0.1);
}

.page-content {
  padding: var(--space-6) 0;
}

.section-title {
  font-size: 1.5rem;
  font-weight: 600;
  margin-bottom: var(--space-4);
  color: var(--text-primary);
}

.search-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-6);
  background-color: var(--surface);
  padding: var(--space-3);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
}

.search-input {
  flex-grow: 1;
}

.sticky-container {
  display: flex;
  flex-direction: column;
  height: calc(100vh - var(--header-height));
  overflow: hidden;
}

.sticky-header {
  position: sticky;
  top: 0;
  z-index: 5;
  background-color: var(--background);
  padding-top: var(--space-4);
  padding-bottom: var(--space-2);
}

.scrollable-content {
  flex: 1;
  overflow-y: auto;
  padding-bottom: var(--space-4);
}

/* Adjust the container to work with sticky headers */
.container.with-sticky {
  display: flex;
  flex-direction: column;
  height: calc(100vh - var(--header-height));
  padding-bottom: 0;
}

"#
);
