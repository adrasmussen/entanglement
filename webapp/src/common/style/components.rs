pub const BASE_COMPONENTS: &str = r#"
/* Base Component Styles */

/* Buttons */
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  font-weight: 500;
  cursor: pointer;
  transition: background-color var(--transition-fast) var(--easing-standard),
              transform var(--transition-fast) var(--easing-standard),
              box-shadow var(--transition-fast) var(--easing-standard);
  border: none;
  outline: none;
}

.btn:focus {
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.3);
}

.btn:active {
  transform: translateY(1px);
}

.btn-primary {
  background-color: var(--primary);
  color: white;
}

.btn-primary:hover {
  background-color: var(--primary-dark);
}

.btn-secondary {
  background-color: var(--neutral-200);
  color: var(--text-primary);
}

.btn-secondary:hover {
  background-color: var(--neutral-300);
}

.btn-danger {
  background-color: var(--error);
  color: white;
}

.btn-danger:hover {
  background-color: #DC2626;
}

.btn-sm {
  padding: var(--space-1) var(--space-3);
  font-size: 0.875rem;
}

.btn-lg {
  padding: var(--space-3) var(--space-5);
  font-size: 1.125rem;
}

/* Cards */
.card {
  background-color: var(--surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  transition: transform var(--transition-normal) var(--easing-standard),
              box-shadow var(--transition-normal) var(--easing-standard);
}

.card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

/* Form Elements */
.form-group {
  margin-bottom: var(--space-4);
}

.form-label {
  display: block;
  margin-bottom: var(--space-2);
  font-weight: 500;
  color: var(--text-secondary);
}

.form-input,
.form-textarea,
.form-select {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background-color: var(--surface);
  color: var(--text-primary);
  transition: border-color var(--transition-fast) var(--easing-standard),
              box-shadow var(--transition-fast) var(--easing-standard);
}

.form-input:focus,
.form-textarea:focus,
.form-select:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.2);
  outline: none;
}

.form-textarea {
  min-height: 100px;
  resize: vertical;
}

/* Media display components */
.media-card {
  display: block;
  background-color: var(--surface);
  border-radius: var(--radius-lg);
  overflow: hidden;
  box-shadow: var(--shadow-sm);
  transition: transform var(--transition-normal) var(--easing-standard),
              box-shadow var(--transition-normal) var(--easing-standard);
}

.media-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

.media-card-image {
  position: relative;
  overflow: hidden;
  width: 100%;
}

.media-card-image img {
  width: 100%;
  aspect-ratio: 4/3;
  object-fit: cover;
  transition: transform var(--transition-slow) var(--easing-standard);
}

.media-card:hover .media-card-image img {
  transform: scale(1.05);
}

.media-card-info {
  padding: var(--space-3);
}

.media-card-info .date {
  font-size: 0.875rem;
  color: var(--text-tertiary);
  margin: 0 0 var(--space-1) 0;
}

.media-card-info .note {
  font-size: 1rem;
  color: var(--text-primary);
  margin: 0;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

/* Standard image display in detail view */

.media-detail-page {
  display: flex;
  gap: var(--space-6);
  position: relative;
  height: calc(100vh - 160px);
}

.media-detail-main {
  flex: 0 0 50%;
  max-width: 50%;
  position: sticky;
  top: var(--header-height);
  height: fit-content;
  max-height: calc(100vh - 140px);
  overflow-y: auto;
  scrollbar-width: none;
}

.media-detail-view {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-bottom: var(--space-4);
}

.media-detail-image {
  max-width: 100%;
  object-fit: contain; /* Maintain aspect ratio */
  border-radius: var(--radius-lg);
  transition: transform var(--transition-normal) var(--easing-standard);
  width: 100%;
  cursor: pointer;
  max-height: calc(100vh - 280px);

}

.media-detail-image:hover {
  transform: scale(1.02);
}

.media-detail-video {
  width: 100%;
  border-radius: var(--radius-lg);
  max-height: calc(100vh - 280px);
}

.media-detail-sidebar {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  overflow-y: auto;
  max-height: calc(100vh - 140px);
  padding-right: var(--space-2);
}

.image-controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  margin-top: var(--space-2);
  color: var(--text-secondary);
  font-size: 0.875rem;
}

.image-controls .btn {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-3);
  font-size: 0.875rem;
}

/* Full size image in modal */
.fullsize-image-container {
  position: relative;
  width: 100%;
  height: 85vh;
  overflow: hidden;
  background-color: rgba(0, 0, 0, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
}

.fullsize-image {
  /* Initial state */
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  cursor: grab;
}

.fullsize-image.panning {
  cursor: grabbing;
}

.fullsize-image.zoomed {
  /* Override contain when zoomed */
  object-fit: none;
}

/* Zoom controls */
.zoom-controls {
  position: absolute;
  bottom: var(--space-4);
  right: var(--space-4);
  display: flex;
  gap: var(--space-2);
  background-color: rgba(0, 0, 0, 0.6);
  padding: var(--space-2);
  border-radius: var(--radius-md);
}

.zoom-button {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background-color: var(--surface);
  color: var(--text-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  font-size: 1.25rem;
  border: none;
  outline: none;
}

.zoom-button:hover {
  background-color: var(--primary-light);
  color: var(--text-inverse);
}

.zoom-level {
  color: white;
  padding: var(--space-2) var(--space-3);
  font-size: 0.875rem;
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .media-detail-image {
    max-height: 70vh;
  }

  .fullsize-image-container {
    height: 75vh;
  }

  .zoom-controls {
    bottom: var(--space-2);
    right: var(--space-2);
  }

  .zoom-button {
    width: 36px;
    height: 36px;
  }
}

/* Skeleton loader */
.skeleton {
  background: linear-gradient(
    90deg,
    var(--neutral-200) 25%,
    var(--neutral-300) 50%,
    var(--neutral-200) 75%
  );
  background-size: 200% 100%;
  animation: skeleton-loading 1.5s infinite;
  border-radius: var(--radius-md);
  height: 1em;
}

@keyframes skeleton-loading {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}

/* Collection Selection Item */
.collection-item {
  padding: var(--space-3);
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  cursor: pointer;
  transition: background-color var(--transition-fast) var(--easing-standard);
}

.collection-item.selected {
  background-color: var(--primary-light);
  color: white;
}

.collection-item:not(.selected):hover {
  background-color: var(--neutral-50);
}

.collection-radio {
  margin-right: var(--space-3);
}

.collection-radio-outer {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid var(--neutral-400);
  display: flex;
  align-items: center;
  justify-content: center;
}

.collection-item.selected .collection-radio-outer {
  border-color: white;
}

.collection-radio-inner {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background-color: white;
}

.collection-info {
  flex: 1;
}

.collection-name {
  font-weight: 500;
}

.collection-meta {
  font-size: 0.875rem;
  color: var(--text-tertiary);
}

.collection-item.selected .collection-meta {
  color: rgba(255, 255, 255, 0.9);
}

.collection-item.error {
  padding: var(--space-3);
  border-bottom: 1px solid var(--border);
  color: var(--error);
}

.collection-item.loading {
  padding: var(--space-3);
  border-bottom: 1px solid var(--border);
}

/* Task Option Selection */
.task-options {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.task-option {
  padding: var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  cursor: pointer;
  transition: all var(--transition-fast) var(--easing-standard);
}

.task-option.selected {
  background-color: var(--primary-light);
  color: white;
  border-color: var(--primary);
  box-shadow: 0 0 0 1px var(--primary);
}

.task-option:not(.selected):hover {
  background-color: var(--neutral-50);
  border-color: var(--neutral-300);
}

.task-radio {
  margin-right: var(--space-3);
}

.task-radio-outer {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid var(--neutral-400);
  display: flex;
  align-items: center;
  justify-content: center;
}

.task-option.selected .task-radio-outer {
  border-color: white;
}

.task-radio-inner {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background-color: white;
}

.task-icon {
  margin-right: var(--space-3);
  font-size: 1.25rem;
}

.task-info {
  flex: 1;
}

.task-name {
  font-weight: 500;
}

.task-description {
  font-size: 0.875rem;
  color: var(--text-tertiary);
}

.task-option.selected .task-description {
  color: rgba(255, 255, 255, 0.9);
}


/* Layout utilities */
.container {
  width: 100%;
  max-width: var(--container-width);
  margin: 0 auto;
  padding: 0 var(--space-4);
}

.grid {
  display: grid;
  gap: var(--space-4);
}

.flex {
  display: flex;
}

.flex-col {
  flex-direction: column;
}

.items-center {
  align-items: center;
}

.justify-between {
  justify-content: space-between;
}

/* Responsive media grid */
.media-grid {
  display: grid;
  gap: var(--space-4);
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
}

@media (max-width: 640px) {
  .media-grid {
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: var(--space-2);
  }
}

/* Table styles */
.table-container {
  width: 100%;
  overflow-x: auto;
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
}

table {
  width: 100%;
  border-collapse: collapse;
}

thead tr {
  background-color: var(--primary);
  color: white;
}

th {
  padding: var(--space-3);
  text-align: left;
  font-weight: 500;
}

tbody tr {
  border-bottom: 1px solid var(--border);
}

tbody tr:nth-child(even) {
  background-color: var(--neutral-50);
}

tbody tr:hover {
  background-color: var(--neutral-100);
}

td {
  padding: var(--space-3);
}

/* Modal styles */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
  animation: fade-in var(--transition-normal) var(--easing-standard);
}

.modal-content {
  background-color: var(--surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  max-width: 90%;
  max-height: 90%;
  overflow: auto;
  animation: slide-up var(--transition-normal) var(--easing-standard);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4);
  border-bottom: 1px solid var(--border);
}

.modal-body {
  padding: var(--space-4);
}

.modal-footer {
  padding: var(--space-4);
  border-top: 1px solid var(--border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.modal-buttons {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-4);
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes slide-up {
  from { transform: translateY(20px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}
"#;
