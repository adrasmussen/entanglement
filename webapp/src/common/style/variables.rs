pub const CSS_VARIABLES: &str = r#"
:root {
  /* Color System */
  --primary: #3B82F6;          /* Primary brand blue */
  --primary-light: #60A5FA;    /* Lighter blue for hover states */
  --primary-dark: #2563EB;     /* Darker blue for active states */
  --secondary: #10B981;        /* Secondary green for success/confirmation */
  --accent: #8B5CF6;           /* Purple accent for highlights */

  /* Neutrals */
  --neutral-50: #F9FAFB;
  --neutral-100: #F3F4F6;
  --neutral-200: #E5E7EB;
  --neutral-300: #D1D5DB;
  --neutral-400: #9CA3AF;
  --neutral-500: #6B7280;
  --neutral-600: #4B5563;
  --neutral-700: #374151;
  --neutral-800: #1F2937;
  --neutral-900: #111827;

  /* Semantic Colors */
  --success: #10B981;
  --warning: #F59E0B;
  --error: #EF4444;
  --info: #3B82F6;

  /* Background and Surface Colors */
  --background: var(--neutral-100);
  --surface: #FFFFFF;
  --surface-raised: #FFFFFF;

  /* Text Colors */
  --text-primary: var(--neutral-900);
  --text-secondary: var(--neutral-600);
  --text-tertiary: var(--neutral-500);
  --text-disabled: var(--neutral-400);
  --text-inverse: #FFFFFF;

  /* Border Colors */
  --border: var(--neutral-200);
  --border-focus: var(--primary);

  /* Layout */
  --header-height: 60px;
  --sidebar-width: 250px;
  --container-width: 1280px;

  /* Spacing System */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --space-12: 48px;
  --space-16: 64px;

  /* Border Radius */
  --radius-sm: 4px;
  --radius-md: 6px;
  --radius-lg: 8px;
  --radius-xl: 12px;
  --radius-full: 9999px;

  /* Shadows */
  --shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
  --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06);
  --shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05);

  /* Animation */
  --transition-fast: 150ms;
  --transition-normal: 250ms;
  --transition-slow: 350ms;
  --easing-standard: cubic-bezier(0.4, 0.0, 0.2, 1);
}"#;
