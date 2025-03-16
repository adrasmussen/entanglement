pub const HOME_STYLES: &str = r#"
/* Modern Home Page Styles */

/* General Layout */
.home-container {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
  }

  section {
    padding: var(--space-12) 0;
  }

  /* Hero Section */
  .hero {
    background: linear-gradient(135deg, var(--primary-dark), var(--accent));
    color: white;
    padding: var(--space-16) 0;
    text-align: center;
  }

  .hero-content {
    max-width: 800px;
    margin: 0 auto;
  }

  .hero-title {
    font-size: 3.5rem;
    font-weight: 700;
    margin-bottom: var(--space-4);
    letter-spacing: -0.02em;
  }

  .hero-subtitle {
    font-size: 1.5rem;
    margin-bottom: var(--space-8);
    opacity: 0.9;
  }

  .hero-actions {
    display: flex;
    gap: var(--space-4);
    justify-content: center;
    margin-top: var(--space-8);
  }

  /* Stats Section */
  .stats-section {
    background-color: var(--surface);
    padding: var(--space-10) 0;
    margin-top: -3rem;
    border-radius: var(--radius-lg) var(--radius-lg) 0 0;
    position: relative;
    z-index: 10;
    box-shadow: var(--shadow-lg);
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--space-6);
  }

  .stat-card {
    background-color: var(--surface-raised);
    padding: var(--space-6);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    display: flex;
    align-items: center;
    position: relative;
    overflow: hidden;
    transition: transform var(--transition-normal) var(--easing-standard),
                box-shadow var(--transition-normal) var(--easing-standard);
  }

  .stat-card:hover {
    transform: translateY(-4px);
    box-shadow: var(--shadow-md);
  }

  .stat-icon {
    width: 60px;
    height: 60px;
    border-radius: var(--radius-full);
    margin-right: var(--space-4);
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-size: 1.5rem;
  }

  .media-icon {
    background-color: var(--primary);
    position: relative;
  }

  .media-icon::before {
    content: "📷";
    font-size: 1.75rem;
  }

  .collection-icon {
    background-color: var(--secondary);
    position: relative;
  }

  .collection-icon::before {
    content: "🖼️";
    font-size: 1.75rem;
  }

  .library-icon {
    background-color: var(--accent);
    position: relative;
  }

  .library-icon::before {
    content: "📚";
    font-size: 1.75rem;
  }

  .stat-content {
    flex: 1;
  }

  .stat-value {
    font-size: 2rem;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }

  .stat-label {
    color: var(--text-secondary);
    margin: 0;
  }

  .stat-action {
    position: absolute;
    bottom: var(--space-3);
    right: var(--space-4);
    color: var(--primary);
    font-size: 0.875rem;
    font-weight: 500;
  }

  /* Features Section */
  .features-section {
    background-color: var(--background);
    padding: var(--space-12) 0;
  }

  .features-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: var(--space-6);
    margin-top: var(--space-8);
  }

  .feature-card {
    background-color: var(--surface);
    padding: var(--space-6);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    text-align: center;
    transition: transform var(--transition-normal) var(--easing-standard),
                box-shadow var(--transition-normal) var(--easing-standard);
  }

  .feature-card:hover {
    transform: translateY(-4px);
    box-shadow: var(--shadow-md);
  }

  .feature-icon {
    width: 64px;
    height: 64px;
    margin: 0 auto var(--space-4);
    border-radius: var(--radius-full);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.75rem;
  }

  .organize-icon {
    background-color: rgba(59, 130, 246, 0.1);
    color: var(--primary);
    position: relative;
  }

  .organize-icon::before {
    content: "🗂️";
    font-size: 1.75rem;
  }

  .search-icon {
    background-color: rgba(16, 185, 129, 0.1);
    color: var(--secondary);
    position: relative;
  }

  .search-icon::before {
    content: "🔍";
    font-size: 1.75rem;
  }

  .secure-icon {
    background-color: rgba(139, 92, 246, 0.1);
    color: var(--accent);
    position: relative;
  }

  .secure-icon::before {
    content: "🔒";
    font-size: 1.75rem;
  }

  .responsive-icon {
    background-color: rgba(245, 158, 11, 0.1);
    color: var(--warning);
    position: relative;
  }

  .responsive-icon::before {
    content: "📱";
    font-size: 1.75rem;
  }

  .feature-title {
    font-size: 1.25rem;
    font-weight: 600;
    margin: var(--space-3) 0;
    color: var(--text-primary);
  }

  .feature-desc {
    color: var(--text-secondary);
    margin: 0;
    line-height: 1.6;
  }

  /* Quick Actions Section */
  .quick-actions {
    background-color: var(--surface);
    padding: var(--space-12) 0;
  }

  .actions-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--space-4);
    margin-top: var(--space-6);
  }

  .quick-action-card {
    background-color: var(--surface-raised);
    padding: var(--space-5);
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: var(--space-3);
    transition: transform var(--transition-normal) var(--easing-standard),
                box-shadow var(--transition-normal) var(--easing-standard);
    box-shadow: var(--shadow-sm);
    cursor: pointer;
    border: none;
    color: var(--text-primary);
    font-family: inherit;
    font-size: 1rem;
    font-weight: 500;
    text-decoration: none;
  }

  .quick-action-card:hover {
    transform: translateY(-4px);
    box-shadow: var(--shadow-md);
    background-color: rgba(59, 130, 246, 0.05);
  }

  .quick-action-icon {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    position: relative;
  }

  .browse-icon::before {
    content: "🖼️";
    font-size: 1.75rem;
  }

  .collections-icon::before {
    content: "📂";
    font-size: 1.75rem;
  }

  .new-collection-icon::before {
    content: "➕";
    font-size: 1.75rem;
  }

  .libraries-icon::before {
    content: "📚";
    font-size: 1.75rem;
  }

  /* Footer */
  .home-footer {
    background-color: var(--neutral-800);
    color: var(--neutral-300);
    padding: var(--space-8) 0;
    text-align: center;
    margin-top: auto;
  }

  /* Responsive Adjustments */
  @media (max-width: 768px) {
    .hero-title {
      font-size: 2.5rem;
    }

    .hero-subtitle {
      font-size: 1.25rem;
    }

    .hero-actions {
      flex-direction: column;
      gap: var(--space-3);
      padding: 0 var(--space-4);
    }

    .stats-section {
      margin-top: -2rem;
    }

    section {
      padding: var(--space-8) 0;
    }
  }

  @media (max-width: 480px) {
    .hero-title {
      font-size: 2rem;
    }

    .stat-card {
      flex-direction: column;
      text-align: center;
    }

    .stat-icon {
      margin-right: 0;
      margin-bottom: var(--space-3);
    }

    .stat-action {
      position: static;
      margin-top: var(--space-3);
    }
  }
"#;
