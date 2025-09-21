class LogViewer {
    constructor() {
        this.currentLogs = [];
        this.currentMetadata = null;
        this.currentPage = 1;
        this.currentFilters = {};
        this.selectedRunId = null;
        
        // Restore state from URL parameters
        const urlParams = this.restoreStateFromUrl();
        
        this.initializeApp();
        this.bindEvents();
        this.setupUrlNavigation();
        this.loadRuseRuns();
        
        // Store URL params for later use (after files are loaded)
        this.pendingUrlParams = urlParams;
        
        // Restore tab selection if specified in URL
        if (urlParams.tab && (urlParams.tab === 'logs' || urlParams.tab === 'results')) {
            this.pendingTab = urlParams.tab;
        }
    }

    initializeApp() {
        // Mermaid is now initialized in the HTML with ESM module
        // No need to initialize here
    }

    bindEvents() {
        // Header actions
        document.getElementById('uploadBtn').addEventListener('click', () => this.showUploadModal());
        document.getElementById('refreshBtn').addEventListener('click', () => this.loadRuseRuns());

        // Tab switching
        document.getElementById('logsTab').addEventListener('click', () => this.switchTab('logs'));
        document.getElementById('resultsTab').addEventListener('click', () => this.switchTab('results'));

        // Filter controls
        document.getElementById('applyFiltersBtn').addEventListener('click', () => this.applyFilters());
        document.getElementById('clearFiltersBtn').addEventListener('click', () => this.clearFilters());
        
        // Search filter - apply on Enter key
        document.getElementById('searchFilter').addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                this.applyFilters();
            }
        });
        
        // Bind "All" checkbox events
        this.bindFilterAllCheckboxes();
        
        // Pagination
        document.getElementById('prevPageBtn').addEventListener('click', () => this.previousPage());
        document.getElementById('nextPageBtn').addEventListener('click', () => this.nextPage());
        document.getElementById('prevPageBtnBottom').addEventListener('click', () => this.previousPage());
        document.getElementById('nextPageBtnBottom').addEventListener('click', () => this.nextPage());
        
        // Items per page selector
        document.getElementById('itemsPerPage').addEventListener('change', (e) => {
            this.currentPage = 1;
            this.updateUrlParams({ page: 1 });
            this.loadLogs('Changing items per page...');
        });

        // Upload modal
        this.bindUploadEvents();
        
        // Modal close events
        document.querySelectorAll('.modal-close').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.target.closest('.modal').classList.remove('active');
            });
        });

        // Mermaid modal toggle button
        document.getElementById('toggleMermaidModal').addEventListener('click', () => {
            FieldsRenderer.toggleMermaidModal();
        });

        // Click outside modal to close
        document.querySelectorAll('.modal').forEach(modal => {
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    modal.classList.remove('active');
                }
            });
        });
    }

    bindFilterAllCheckboxes() {
        // Level filter
        const levelFilterAll = document.getElementById('levelFilterAll');
        const levelFilterOptions = document.getElementById('levelFilterOptions');
        
        levelFilterAll.addEventListener('change', (e) => {
            const checkboxes = levelFilterOptions.querySelectorAll('input[type="checkbox"]');
            checkboxes.forEach(checkbox => {
                checkbox.checked = e.target.checked;
            });
            this.updateFilterAllState('levelFilterAll', levelFilterOptions);
        });

        // Target filter
        const targetFilterAll = document.getElementById('targetFilterAll');
        const targetFilterOptions = document.getElementById('targetFilterOptions');
        
        targetFilterAll.addEventListener('change', (e) => {
            const checkboxes = targetFilterOptions.querySelectorAll('input[type="checkbox"]');
            checkboxes.forEach(checkbox => {
                checkbox.checked = e.target.checked;
            });
            this.updateFilterAllState('targetFilterAll', targetFilterOptions);
        });

        // Thread ID filter
        const threadIdFilterAll = document.getElementById('threadIdFilterAll');
        const threadIdFilterOptions = document.getElementById('threadIdFilterOptions');
        
        threadIdFilterAll.addEventListener('change', (e) => {
            const checkboxes = threadIdFilterOptions.querySelectorAll('input[type="checkbox"]');
            checkboxes.forEach(checkbox => {
                checkbox.checked = e.target.checked;
            });
            this.updateFilterAllState('threadIdFilterAll', threadIdFilterOptions);
        });

        // Task filter
        const taskFilterAll = document.getElementById('taskFilterAll');
        const taskFilterOptions = document.getElementById('taskFilterOptions');
        
        taskFilterAll.addEventListener('change', (e) => {
            const checkboxes = taskFilterOptions.querySelectorAll('input[type="checkbox"]');
            checkboxes.forEach(checkbox => {
                checkbox.checked = e.target.checked;
            });
            this.updateFilterAllState('taskFilterAll', taskFilterOptions);
        });

        // Iteration filter
        const iterationFilterAll = document.getElementById('iterationFilterAll');
        const iterationFilterOptions = document.getElementById('iterationFilterOptions');
        
        iterationFilterAll.addEventListener('change', (e) => {
            const checkboxes = iterationFilterOptions.querySelectorAll('input[type="checkbox"]');
            checkboxes.forEach(checkbox => {
                checkbox.checked = e.target.checked;
            });
            this.updateFilterAllState('iterationFilterAll', iterationFilterOptions);
        });

        // Bind individual checkbox events to update "All" state
        this.bindIndividualFilterCheckboxes();
    }

    bindIndividualFilterCheckboxes() {
        // Bind events to level filter checkboxes
        const levelFilterOptions = document.getElementById('levelFilterOptions');
        if (levelFilterOptions) {
            levelFilterOptions.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
                checkbox.addEventListener('change', () => {
                    this.updateFilterAllState('levelFilterAll', levelFilterOptions);
                });
            });
        }

        // Bind events to target filter checkboxes
        const targetFilterOptions = document.getElementById('targetFilterOptions');
        if (targetFilterOptions) {
            targetFilterOptions.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
                checkbox.addEventListener('change', () => {
                    this.updateFilterAllState('targetFilterAll', targetFilterOptions);
                });
            });
        }

        // Bind events to thread ID filter checkboxes
        const threadIdFilterOptions = document.getElementById('threadIdFilterOptions');
        if (threadIdFilterOptions) {
            threadIdFilterOptions.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
                checkbox.addEventListener('change', () => {
                    this.updateFilterAllState('threadIdFilterAll', threadIdFilterOptions);
                });
            });
        }

        // Bind events to task filter checkboxes
        const taskFilterOptions = document.getElementById('taskFilterOptions');
        if (taskFilterOptions) {
            taskFilterOptions.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
                checkbox.addEventListener('change', () => {
                    this.updateFilterAllState('taskFilterAll', taskFilterOptions);
                    this.handleTaskFilterChange();
                });
            });
        }

        // Bind events to iteration filter checkboxes
        const iterationFilterOptions = document.getElementById('iterationFilterOptions');
        if (iterationFilterOptions) {
            iterationFilterOptions.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
                checkbox.addEventListener('change', () => {
                    this.updateFilterAllState('iterationFilterAll', iterationFilterOptions);
                });
            });
        }
    }

    handleTaskFilterChange() {
        const selectedTasks = this.getCheckedFilterValues('taskFilterOptions');
        let max_iterations = 0;
        let task_metadata_logs = 0;
        let iteration_stats = {};
        
        for (const taskName of selectedTasks) {
            if (!taskName || !this.currentMetadata?.stats?.tasks?.[taskName]) {
                continue;
            }
            
            // Skip "metadata" task as it doesn't have iterations
            if (taskName === 'metadata') {
                continue;
            }
            
            const iterations = this.currentMetadata.stats.tasks[taskName].iterations;
            task_metadata_logs += this.currentMetadata.stats.tasks[taskName].metadata_logs;
            max_iterations = Math.max(max_iterations, Object.keys(iterations).length);
            for (const iteration of Object.keys(iterations)) {
                iteration_stats[iteration] = (iteration_stats[iteration] || 0) + iterations[iteration];
            }
        }
        
        // Always show iteration filter if there are tasks with iterations
        if (max_iterations > 0) {
            document.getElementById('iterationFilterGroup').style.display = 'block';
            this.populateIterationFilter(task_metadata_logs, iteration_stats);
        } else {
            document.getElementById('iterationFilterGroup').style.display = 'none';
        }
    }

    populateIterationFilter(metadata_logs, iteration_stats) {
        const iterationFilterOptions = document.getElementById('iterationFilterOptions');
        if (!iterationFilterOptions) return;
        
        // Store current selections
        const currentIterations = this.getCheckedFilterValues('iterationFilterOptions');
        
        // Clear and repopulate
        iterationFilterOptions.innerHTML = '';
        
        const label = document.createElement('label');
        label.className = 'checkbox-label';
        
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.value = 'metadata';
        checkbox.dataset.filterType = 'iteration';
        
        const span = document.createElement('span');
        span.textContent = `Metadata (${metadata_logs})`;
        
        label.appendChild(checkbox);
        label.appendChild(span);
        iterationFilterOptions.appendChild(label);
        
        // Add other iterations (excluding "metadata")
        Object.entries(iteration_stats).forEach(([iteration, count]) => {
            if (iteration === 'metadata') return; // Skip as it's already added
            
            const label = document.createElement('label');
            label.className = 'checkbox-label';
            
            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.value = iteration;
            checkbox.dataset.filterType = 'iteration';
            
            const span = document.createElement('span');
            span.textContent = `${iteration} (${count})`;
            
            label.appendChild(checkbox);
            label.appendChild(span);
            iterationFilterOptions.appendChild(label);
        });
        
        // Restore selections
        this.restoreFilterSelections('iterationFilterOptions', currentIterations);
        
        // Update "All" checkbox state
        this.updateFilterAllState('iterationFilterAll', iterationFilterOptions);
        
        // Bind events to new checkboxes
        iterationFilterOptions.querySelectorAll('input[type="checkbox"]').forEach(checkbox => {
            checkbox.addEventListener('change', () => {
                this.updateFilterAllState('iterationFilterAll', iterationFilterOptions);
            });
        });
    }

    updateFilterAllState(allCheckboxId, optionsContainer) {
        const allCheckbox = document.getElementById(allCheckboxId);
        const checkboxes = optionsContainer.querySelectorAll('input[type="checkbox"]');
        const checkedCount = Array.from(checkboxes).filter(cb => cb.checked).length;
        
        if (checkedCount === 0) {
            allCheckbox.checked = false;
            allCheckbox.indeterminate = false;
        } else if (checkedCount === checkboxes.length) {
            allCheckbox.checked = true;
            allCheckbox.indeterminate = false;
        } else {
            allCheckbox.checked = false;
            allCheckbox.indeterminate = true;
        }
    }

    bindUploadEvents() {
        const logUploadArea = document.getElementById('logUploadArea');
        const resultUploadArea = document.getElementById('resultUploadArea');
        const logFileInput = document.getElementById('logFileInput');
        const resultFileInput = document.getElementById('resultFileInput');

        // Log file upload area
        logUploadArea.addEventListener('click', () => logFileInput.click());
        
        logUploadArea.addEventListener('dragover', (e) => {
            e.preventDefault();
            logUploadArea.classList.add('dragover');
        });

        logUploadArea.addEventListener('dragleave', () => {
            logUploadArea.classList.remove('dragover');
        });

        logUploadArea.addEventListener('drop', (e) => {
            e.preventDefault();
            logUploadArea.classList.remove('dragover');
            const files = e.dataTransfer.files;
            if (files.length > 0) {
                logFileInput.files = files;
            }
        });

        logFileInput.addEventListener('change', (e) => {
            // File input change handled by upload button
        });

        // Result file upload area
        resultUploadArea.addEventListener('click', () => resultFileInput.click());
        
        resultUploadArea.addEventListener('dragover', (e) => {
            e.preventDefault();
            resultUploadArea.classList.add('dragover');
        });

        resultUploadArea.addEventListener('dragleave', () => {
            resultUploadArea.classList.remove('dragover');
        });

        resultUploadArea.addEventListener('drop', (e) => {
            e.preventDefault();
            resultUploadArea.classList.remove('dragover');
            const files = e.dataTransfer.files;
            if (files.length > 0) {
                resultFileInput.files = files;
            }
        });

        resultFileInput.addEventListener('change', (e) => {
            // File input change handled by upload button
        });

        // Upload button
        const uploadBtn = document.querySelector('#uploadModal .btn-primary');
        if (uploadBtn) {
            uploadBtn.addEventListener('click', () => this.uploadFiles());
        }
    }

    setupUrlNavigation() {
        // Handle browser back/forward navigation
        window.addEventListener('popstate', (event) => {
            const params = this.getUrlParams();
            
            // Update state without pushing new history
            if (params.run !== this.selectedRunId) {
                this.selectedRunId = params.run;
                if (params.run) {
                    // Find and select the run item
                    const targetItem = document.querySelector(`[data-run-id="${params.run}"]`);
                    if (targetItem) {
                        // Update UI without triggering URL update again
                        this.selectRuseRun(params.run);
                    }
                }
            }
            
            if (params.page !== this.currentPage) {
                this.currentPage = params.page;
                this.loadLogs();
            }

            // Handle tab selection from URL
            if (params.tab && (params.tab === 'logs' || params.tab === 'results')) {
                this.switchTab(params.tab);
            }
        });
    }

    /**
     * Load available Ruse runs
     */
    async loadRuseRuns() {
        try {
            const response = await fetch('/api/runs');
            const data = await response.json();
            
            this.displayRuseRuns(data.runs);
        } catch (error) {
            console.error('Error loading Ruse runs:', error);
            this.showError('Failed to load Ruse runs');
        }
    }

    /**
     * Display Ruse runs in the sidebar
     */
    displayRuseRuns(runs) {
        const container = document.getElementById('ruseRunsList');
        
        if (!runs || runs.length === 0) {
            container.innerHTML = '<div class="no-runs">No Ruse runs found</div>';
            return;
        }

        container.innerHTML = runs.map(run => `
            <div class="ruse-run-item" data-run-id="${run.id}">
                <div class="ruse-run-time">${this.formatRunTime(run.runTime)}</div>
                <div class="ruse-run-stats">
                    <span class="ruse-run-stat">📊 ${run.taskCount} tasks</span>
                    <span class="ruse-run-stat">✅ ${run.passedTasks}</span>
                    <span class="ruse-run-stat">❌ ${run.failedTasks}</span>
                    <span class="ruse-run-stat">⏱️ ${run.totalTime.toFixed(2)}s</span>
                </div>
            </div>
        `).join('');

        // Bind click events
        container.querySelectorAll('.ruse-run-item').forEach(item => {
            item.addEventListener('click', () => this.selectRuseRun(item.dataset.runId));
        });
        
        // Auto-select run from URL parameters if specified
        if (this.pendingUrlParams && this.pendingUrlParams.run) {
            const targetItem = container.querySelector(`[data-run-id="${this.pendingUrlParams.run}"]`);
            if (targetItem) {
                this.selectRuseRun(this.pendingUrlParams.run);
            }
            this.pendingUrlParams = null; // Clear after use
        }
    }

    /**
     * Format run time for display
     */
    formatRunTime(runTime) {
        if (typeof runTime === 'string') {
            try {
                const date = new Date(runTime);
                return date.toLocaleString("en-GB");
            } catch (e) {
                return runTime;
            }
        }
        return 'Unknown time';
    }

    /**
     * Select a Ruse run
     */
    async selectRuseRun(runId) {
        // Update UI selection
        document.querySelectorAll('.ruse-run-item').forEach(item => {
            item.classList.remove('selected');
        });
        document.querySelector(`[data-run-id="${runId}"]`).classList.add('selected');

        // Store current run ID
        this.selectedRunId = runId;

        // Update URL parameters
        this.updateUrlParams({
            run: runId,
            page: 1
        });

        // Show run container and hide welcome screen
        document.getElementById('welcomeScreen').style.display = 'none';
        document.getElementById('runContainer').style.display = 'block';

        // Load run metadata
        await this.loadRunMetadata(runId);
        
        // Load the pending tab from URL or default to results
        const tabToLoad = this.pendingTab || 'results';
        this.pendingTab = null; // Clear after use
        this.switchTab(tabToLoad);
    }

    /**
     * Load metadata for a specific run
     */
    async loadRunMetadata(runId) {
        try {
            const response = await fetch(`/api/logs/${runId}`);
            const data = await response.json();
            
            this.currentMetadata = data.metadata;
            this.updateRunHeader();
        } catch (error) {
            console.error('Error loading run metadata:', error);
        }
    }

    /**
     * Update the run header with metadata
     */
    updateRunHeader() {
        if (!this.currentMetadata) return;

        const runTime = document.getElementById('runTime');
        const runStats = document.getElementById('runStats');

        if (this.currentMetadata.runTime) {
            runTime.textContent = this.formatRunTime(this.currentMetadata.runTime);
        } else {
            runTime.textContent = 'Unknown time';
        }

        const stats = this.currentMetadata.resultMetadata;
        runStats.innerHTML = `
            ${stats.taskCount} tasks • 
            ${stats.passedTasks} passed • 
            ${stats.failedTasks} failed • 
            ${stats.totalTime.toFixed(2)}s total
        `;
    }

    /**
     * Switch between logs and results tabs
     */
    switchTab(tabName) {
        // Update tab buttons
        document.querySelectorAll('.tab-btn').forEach(btn => {
            btn.classList.remove('active');
        });
        document.getElementById(`${tabName}Tab`).classList.add('active');

        // Update tab content
        document.querySelectorAll('.tab-content').forEach(content => {
            content.classList.remove('active');
        });
        document.getElementById(`${tabName}TabContent`).classList.add('active');

        // Update URL with current tab
        if (this.selectedRunId) {
            this.updateUrlParams({
                run: this.selectedRunId,
                tab: tabName,
                page: this.currentPage
            });
        }

        if (tabName === 'logs') {
            this.loadLogs();
        } else if (tabName === 'results') {
            this.loadResults();
        }
    }

    /**
     * Load results for the current run
     */
    async loadResults() {
        if (!this.selectedRunId) return;

        const resultsContent = document.getElementById('resultsEntries');
        const loadingOverlay = document.getElementById('resultsLoadingOverlay');

        try {
            loadingOverlay.classList.remove('hidden');
            
            const response = await fetch(`/api/runs/${this.selectedRunId}/results`);
            const data = await response.json();
            
            this.displayResults(data);
        } catch (error) {
            console.error('Error loading results:', error);
            resultsContent.innerHTML = '<div class="error">Failed to load results</div>';
        } finally {
            loadingOverlay.classList.add('hidden');
        }
    }

    /**
     * Display results data
     */
    displayResults(data) {
        const resultsContent = document.getElementById('resultsEntries');
        
        if (!data.results || !data.results.tasks) {
            resultsContent.innerHTML = '<div class="no-results">No results data available</div>';
            return;
        }

        const { metadata, results, runInfo } = data;
        
        resultsContent.innerHTML = `
            <div class="results-overview">
                <div class="results-stats">
                    <div class="stat-item">
                        <div class="stat-label">Total Tasks</div>
                        <div class="stat-value">${metadata.taskCount}</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-label">Passed</div>
                        <div class="stat-value">${metadata.passedTasks}</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-label">Failed</div>
                        <div class="stat-value">${metadata.failedTasks}</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-label">Total Time</div>
                        <div class="stat-value">${metadata.totalTime.toFixed(2)}s</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-label">Average Time</div>
                        <div class="stat-value">${metadata.averageTime.toFixed(2)}s</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-label">Max Depth</div>
                        <div class="stat-value">${metadata.maxDepth}</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-label">Max Size</div>
                        <div class="stat-value">${metadata.maxSize}</div>
                    </div>
                </div>
                
                <div class="system-info">
                    <h4>System Information</h4>
                    <div class="info-grid">
                        <div class="info-item">
                            <span class="info-label">OS:</span>
                            <span class="info-value">${results.metadata.sysinfo?.name || 'Unknown'} ${results.metadata.sysinfo?.os || ''}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">Kernel:</span>
                            <span class="info-value">${results.metadata.sysinfo?.kernel || 'Unknown'}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">CPU:</span>
                            <span class="info-value">${results.metadata.sysinfo?.cpu || 'Unknown'}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">Cores:</span>
                            <span class="info-value">${results.metadata.sysinfo?.cpu_core_count || 'Unknown'}</span>
                        </div>
                    </div>
                </div>
                
                <div class="config-section">
                    <h4>Configuration</h4>
                    <div class="config-summary">
                        <div class="config-item">
                            <span class="config-label">Timeout:</span>
                            <span class="config-value">${results.metadata.config?.timeout || 'Unknown'}s</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Max Iterations:</span>
                            <span class="config-value">${results.metadata.config?.max_iterations || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Multi-thread:</span>
                            <span class="config-value">${results.metadata.config?.multi_thread ? 'Yes' : 'No'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Max Mutations:</span>
                            <span class="config-value">${results.metadata.config?.max_mutations || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Iteration Workers:</span>
                            <span class="config-value">${results.metadata.config?.iteration_workers_count || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Max Task Memory:</span>
                            <span class="config-value">${results.metadata.config?.max_task_mem || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Bank Type:</span>
                            <span class="config-value">${Object.keys(results.metadata.config?.bank_config)[0] || 'Unknown'}</span>
                        </div>
                    </div>
                                        
                    <button class="config-expand-btn">▼ Show Details</button>
                    <div class="config-details" style="display: none;">
                        <h5>Full Configuration</h5>
                        <div class="full-config">
                            <pre class="config-json">${JSON.stringify(results.metadata.config, null, 2)}</pre>
                        </div>
                    </div>
                </div>
            </div>
            
            <div class="tasks-section">
                <h4>Tasks (${results.tasks.length})</h4>
                <div class="tasks-list">
                    ${results.tasks.map((task, index) => `
                        <div class="task-item" data-task-index="${index}">
                            <div class="task-header">
                                <div class="task-name">${task.path.split('/').pop()}</div>
                                <div class="task-status ${task.error ? 'failed' : 'passed'}">
                                    ${task.error ? '❌ Failed' : '✅ Passed'}
                                </div>
                                <div class="task-stat">${(task.iterations.length)} iterations</div>
                                <div class="task-stat">${(task.total_statistics.Evaluated)} evaluated</div>
                                <div class="task-time">${this.formatTaskTime(task.total_time)}</div>
                                <button class="task-expand-btn">▼</button>
                            </div>
                            <div class="task-details" style="display: none;">
                                <div class="task-info">
                                    <div class="task-path">Path: ${task.path}</div>
                                    <div class="task-opcodes">Opcodes: ${task.opcode_count}</div>
                                    <div class="task-stats">
                                        <span>Evaluated: ${task.total_statistics.Evaluated}</span>
                                        <span>Bank Size: ${task.total_statistics.BankSize}</span>
                                        <span>Max Depth: ${task.total_statistics.MaxDepth}</span>
                                        <span>Max Mutations: ${task.total_statistics.MaxMutatingOpcodes}</span>
                                        <span>Max Size: ${task.total_statistics.MaxSize}</span>
                                        <span>Found Context: ${task.total_statistics.FoundContextCount}</span>
                                    </div>

                                    ${task.found ? `<div class="task-found">Found: ${task.found}</div>` : ''}
                                    ${task.error ? `<div class="task-error">Error: ${task.error}</div>` : ''}
                                </div>
                                <div class="task-iterations">
                                    <h5>Iterations (${task.iterations.length})</h5>
                                    ${task.iterations.map((iter, iterIndex) => `
                                        <div class="iteration-item">
                                            <div class="iteration-header">
                                                <span class="iteration-number">#${iterIndex + 1}</span>
                                                <span class="iteration-time">${this.formatTaskTime(iter.time)}</span>
                                            </div>
                                            <div class="iteration-stats">
                                                <span>Evaluated: ${iter.statistics.Evaluated}</span>
                                                <span>Bank Size: ${iter.statistics.BankSize}</span>
                                                <span>Max Depth: ${iter.statistics.MaxDepth}</span>
                                                <span>Max Mutations: ${iter.statistics.MaxMutatingOpcodes}</span>
                                                <span>Max Size: ${iter.statistics.MaxSize}</span>
                                                <span>Found Context: ${iter.statistics.FoundContextCount}</span>
                                            </div>
                                        </div>
                                    `).join('')}
                                </div>
                            </div>
                        </div>
                    `).join('')}
                </div>
            </div>
        `;

        // Bind task expansion events
        resultsContent.querySelectorAll('.task-expand-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                const taskItem = btn.closest('.task-item');
                const details = taskItem.querySelector('.task-details');
                const isExpanded = details.style.display !== 'none';
                
                details.style.display = isExpanded ? 'none' : 'block';
                btn.textContent = isExpanded ? '▼' : '▲';
            });
        });

        // Bind config expansion events
        resultsContent.querySelectorAll('.config-expand-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                const configSection = btn.closest('.config-section');
                const details = configSection.querySelector('.config-details');
                const isExpanded = details.style.display !== 'none';
                
                details.style.display = isExpanded ? 'none' : 'block';
                btn.textContent = isExpanded ? '▼ Show Details' : '▲ Hide Details';
            });
        });
    }

    /**
     * Format task time for display
     */
    formatTaskTime(time) {
        if (!time) return '0s';
        const seconds = time.secs || 0;
        const nanos = time.nanos || 0;
        const totalSeconds = seconds + (nanos / 1000000000);
        return totalSeconds.toFixed(3) + 's';
    }

    async selectLogFile(item) {
        const runId = item.dataset.runId;
        const uploadedFile = item.dataset.uploadedFile;

        if (runId) {
            // Load cached file
            this.selectedRunId = runId;
            
            // Only reset page to 1 if this is a user-initiated selection (not from URL)
            const fromUrl = this.pendingUrlParams && this.pendingUrlParams.file === runId;
            if (!fromUrl) {
                this.currentPage = 1;
            }
            
            // Update URL parameters
            this.updateUrlParams({
                run: runId,
                page: this.currentPage
            });
            
            await this.loadLogs('Loading selected file...');
            await this.loadStats();
        } else if (uploadedFile) {
            // Process uploaded file
            await this.processUploadedFile(uploadedFile);
        }

        this.updateFileSelection(item, true);
    }

    updateFileSelection(item, updateUrl = true) {
        // Update active state
        document.querySelectorAll('.log-file-item').forEach(i => i.classList.remove('active'));
        item.classList.add('active');
    }

    async processUploadedFile(filePath) {
        try {
            this.showLoading('Processing file...');
            
            const response = await fetch('/api/process', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ filePath })
            });

            if (!response.ok) {
                throw new Error('Failed to process file');
            }

            const result = await response.json();
            this.showSuccess('File processed successfully!');
            
            // Reload file list to show the new cached file
            await this.loadLogFiles();
            
        } catch (error) {
            console.error('Error processing file:', error);
            this.showError('Failed to process file');
        }
    }

    async loadLogs(loadingMessage = 'Loading logs...') {
        if (!this.selectedRunId) return;

        try {
            // Show loading animation
            this.showLogsLoading(loadingMessage);
            
            const itemsPerPage = parseInt(document.getElementById('itemsPerPage').value) || 25;
            const params = new URLSearchParams({
                page: this.currentPage,
                limit: itemsPerPage,
                ...this.currentFilters
            });

            const response = await fetch(`/api/logs/${this.selectedRunId}?${params}`);
            const data = await response.json();
            
            this.currentLogs = data.logs;
            this.currentMetadata = data.metadata;
            
            this.renderLogs(data);
            this.updatePagination(data.pagination);
            this.populateFilterOptions(data.metadata.stats);
            
            // Show logs container, hide welcome screen
            document.getElementById('welcomeScreen').style.display = 'none';
            document.getElementById('runContainer').style.display = 'block';
            
            // Hide loading animation
            this.hideLogsLoading();
            
        } catch (error) {
            console.error('Error loading logs:', error);
            this.hideLogsLoading();
            this.showError('Failed to load logs');
        }
    }

    renderLogs(data) {
        const entries = document.getElementById('logsEntries');
        const fileName = document.getElementById('currentFileName');

        fileName.textContent = this.getFileNameFromPath(data.metadata.sourceFile);

        if (data.logs.length === 0) {
            entries.innerHTML = '<div class="loading">No logs found matching the current filters</div>';
            return;
        }

        let html = '';
        data.logs.forEach((log, index) => {
            html += this.renderLogEntry(log, index);
        });

        entries.innerHTML = html;

        // Bind click events for log expansion
        entries.querySelectorAll('.log-header').forEach((header, index) => {
            header.addEventListener('click', () => this.toggleLogEntry(index));
        });

        // Bind click events for Mermaid diagrams
        entries.querySelectorAll('.mermaid-diagram').forEach((diagram) => {
            diagram.addEventListener('click', (e) => {
                e.stopPropagation();
                // Get the Mermaid text from the corresponding raw text element
                const mermaidId = diagram.id.replace('diagram-', '');
                const rawElement = document.getElementById(`raw-${mermaidId}`);
                console.log(mermaidId, rawElement);
                if (rawElement) {
                    const mermaidRawText = rawElement.querySelector('.mermaid-raw-text');
                    if (mermaidRawText) {
                        console.log(mermaidRawText.textContent);
                        FieldsRenderer.showMermaidModal(mermaidId, mermaidRawText.textContent);
                    }
                }
            });
        });
    }

    renderLogEntry(log, index) {
        const timestamp = new Date(log.timestamp).toLocaleString("en-GB");
        const level = log.level || 'unknown';
        const message = log.fields?.message || 'No message';
        const isPanic = log._meta.isPanic;

        
        let levelClass = level.toLowerCase();
        if (isPanic) levelClass = 'panic';

        return `
            <div class="log-entry ${levelClass}" data-index="${index}">
                <div class="log-header">
                    <div class="log-header-top">
                        <div class="log-meta">
                            <span class="log-level ${levelClass}">${level}${isPanic ? ' 🚨' : ''}</span>
                            <span class="log-timestamp">${timestamp}</span>
                            <div class="log-badges"></div>
                        </div>
                        <div class="log-location">
                            <span class="log-target">${log.target || 'unknown'}</span>
                            <br>
                            ${log.filename}:${log.line_number} • Thread: ${log.threadId || 'unknown'}
                        </div>
                    </div>
                    <div class="log-header-message">${escapeHtml(message)}</div>
                    ${this.renderTaskInfo(log)}
                </div>
                <div class="log-body" id="log-body-${index}">
                    <div class="log-message">${escapeHtml(message)}</div>
                    ${FieldsRenderer.renderLogFields(log)}
                    ${FieldsRenderer.renderExtensions(log._meta.extensions, index)}
                    ${isPanic ? FieldsRenderer.renderPanicInfo(log._meta) : ''}
                </div>
            </div>
        `;
    }

    renderTaskInfo(log) {
        let html = '';
        
        // Task information from _meta.span
        if (log._meta && log._meta.span) {
            if (log._meta.span.task) {
                html += `<div class="log-task">
                    <span class="task-label">Task:</span>
                    <span class="task-name">${escapeHtml(log._meta.span.task)}</span>
                    ${(log._meta.span.iteration != null) ? `<span class="task-iteration">(iteration: ${log._meta.span.iteration})</span>` : ''}
                </div>`;
            } else {
                // Log without task belongs to "metadata" category
                html += `<div class="log-task">
                    <span class="task-label">Task:</span>
                    <span class="task-name metadata-task">Metadata</span>
                </div>`;
            }
        }
        
        return html;
    }

    toggleLogEntry(index) {
        const body = document.getElementById(`log-body-${index}`);
        body.classList.toggle('expanded');
        
        // Re-run Mermaid for newly expanded entries
        if (body.classList.contains('expanded')) {
            // Only render Mermaid diagrams in this specific expanded log entry
            setTimeout(() => FieldsRenderer.triggerMermaidRendering(body), 100);
        }
    }

    populateFilterOptions(stats) {
        // Store current filter values before repopulating
        const currentLevels = this.getCheckedFilterValues('levelFilterOptions');
        const currentTargets = this.getCheckedFilterValues('targetFilterOptions');
        const currentThreadIds = this.getCheckedFilterValues('threadIdFilterOptions');
        const currentTasks = this.getCheckedFilterValues('taskFilterOptions');
        const currentIterations = this.getCheckedFilterValues('iterationFilterOptions');

        // Populate level filter
        const levelFilterOptions = document.getElementById('levelFilterOptions');
        levelFilterOptions.innerHTML = '';
        Object.entries(stats.levels).forEach(([level, count]) => {
            const label = document.createElement('label');
            label.className = 'checkbox-label';
            
            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.value = level;
            checkbox.dataset.filterType = 'level';
            
            const span = document.createElement('span');
            span.textContent = `${level} (${count})`;
            
            label.appendChild(checkbox);
            label.appendChild(span);
            levelFilterOptions.appendChild(label);
        });

        // Populate target filter
        const targetFilterOptions = document.getElementById('targetFilterOptions');
        targetFilterOptions.innerHTML = '';
        Object.entries(stats.targets).forEach(([target, count]) => {
            const label = document.createElement('label');
            label.className = 'checkbox-label';
            
            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.value = target;
            checkbox.dataset.filterType = 'target';
            
            const span = document.createElement('span');
            span.textContent = `${target} (${count})`;
            
            label.appendChild(checkbox);
            label.appendChild(span);
            targetFilterOptions.appendChild(label);
        });

        // Populate thread ID filter
        const threadIdFilterOptions = document.getElementById('threadIdFilterOptions');
        threadIdFilterOptions.innerHTML = '';
        let sortedThreadIds = Object.entries(stats.threadIds).sort(([keyA, _valueA], [keyB, _valueB]) => keyA.localeCompare(keyB));
        sortedThreadIds.forEach(([threadId, count]) => {
            const label = document.createElement('label');
            label.className = 'checkbox-label';
            
            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.value = threadId;
            checkbox.dataset.filterType = 'threadId';
            
            const span = document.createElement('span');
            span.textContent = `${threadId} (${count})`;
            
            label.appendChild(checkbox);
            label.appendChild(span);
            threadIdFilterOptions.appendChild(label);
        });

        // Populate tasks filter
        const taskFilterOptions = document.getElementById('taskFilterOptions');
        taskFilterOptions.innerHTML = '';
        Object.entries(stats.tasks).forEach(([task, taskData]) => {
            const label = document.createElement('label');
            label.className = 'checkbox-label';
            
            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.value = task;
            checkbox.dataset.filterType = 'task';
            
            const span = document.createElement('span');
            // Special handling for "metadata" task
            if (task === 'metadata') {
                span.textContent = `Metadata (${taskData.count})`;
            } else {
                span.textContent = `${task} (${taskData.count})`;
            }
            
            label.appendChild(checkbox);
            label.appendChild(span);
            taskFilterOptions.appendChild(label);
        });
        
        // Restore selected values if they still exist in the new options
        this.restoreFilterSelections('levelFilterOptions', currentLevels);
        this.restoreFilterSelections('targetFilterOptions', currentTargets);
        this.restoreFilterSelections('threadIdFilterOptions', currentThreadIds);
        this.restoreFilterSelections('taskFilterOptions', currentTasks);
        
        // Update "All" checkbox states
        this.updateFilterAllState('levelFilterAll', levelFilterOptions);
        this.updateFilterAllState('targetFilterAll', targetFilterOptions);
        this.updateFilterAllState('threadIdFilterAll', threadIdFilterOptions);
        this.updateFilterAllState('taskFilterAll', taskFilterOptions);
        
        // Bind individual checkbox events
        this.bindIndividualFilterCheckboxes();
    }

    getCheckedFilterValues(containerId) {
        const container = document.getElementById(containerId);
        if (!container) return [];
        
        const checkboxes = container.querySelectorAll('input[type="checkbox"]:checked');
        return Array.from(checkboxes).map(cb => cb.value);
    }

    restoreFilterSelections(containerId, selectedValues) {
        if (!selectedValues || selectedValues.length === 0) return;
        
        const container = document.getElementById(containerId);
        if (!container) return;
        
        selectedValues.forEach(value => {
            const checkbox = container.querySelector(`input[value="${value}"]`);
            if (checkbox) {
                checkbox.checked = true;
            }
        });
    }

    applyFilters() {
        const filters = {};

        const level = this.getCheckedFilterValues('levelFilterOptions');
        if (level.length > 0) filters.level = level;

        const target = this.getCheckedFilterValues('targetFilterOptions');
        if (target.length > 0) filters.target = target;

        const threadId = this.getCheckedFilterValues('threadIdFilterOptions');
        if (threadId.length > 0) filters.threadId = threadId;

        const search = document.getElementById('searchFilter').value.trim();
        if (search) filters.search = RegExp.escape(search);

        const isPanic = document.getElementById('panicFilter').checked;
        if (isPanic) filters.isPanic = 'true';

        // Add task filtering
        const taskFilter = this.getCheckedFilterValues('taskFilterOptions');
        if (taskFilter.length > 0) {
            const iterationFilter = this.getCheckedFilterValues('iterationFilterOptions');
            if (iterationFilter.length > 0) {
                filters.task = taskFilter;
                filters.iteration = iterationFilter;
            } else {
                filters.task = taskFilter;
            }
        }

        this.currentFilters = filters;
        this.currentPage = 1;
        this.updateUrlParams({ page: 1 });
        this.loadLogs('Applying filters...');
    }

    clearFilters() {
        // Clear all filter checkboxes
        this.clearFilterCheckboxes('levelFilterOptions');
        this.clearFilterCheckboxes('targetFilterOptions');
        this.clearFilterCheckboxes('threadIdFilterOptions');
        this.clearFilterCheckboxes('taskFilterOptions');
        this.clearFilterCheckboxes('iterationFilterOptions');
        
        // Reset "All" checkboxes
        document.getElementById('levelFilterAll').checked = false;
        document.getElementById('levelFilterAll').indeterminate = false;
        document.getElementById('targetFilterAll').checked = false;
        document.getElementById('targetFilterAll').indeterminate = false;
        document.getElementById('threadIdFilterAll').checked = false;
        document.getElementById('threadIdFilterAll').indeterminate = false;
        document.getElementById('taskFilterAll').checked = false;
        document.getElementById('taskFilterAll').indeterminate = false;
        document.getElementById('iterationFilterAll').checked = false;
        document.getElementById('iterationFilterAll').indeterminate = false;
        
        // Clear other filters
        document.getElementById('searchFilter').value = '';
        document.getElementById('panicFilter').checked = false;

        this.currentFilters = {};
        this.currentPage = 1;
        this.updateUrlParams({ page: 1 });
        this.loadLogs('Clearing filters...');
    }

    clearFilterCheckboxes(containerId) {
        const container = document.getElementById(containerId);
        if (!container) return;
        
        const checkboxes = container.querySelectorAll('input[type="checkbox"]');
        checkboxes.forEach(checkbox => {
            checkbox.checked = false;
        });
    }

    updatePagination(pagination) {
        // Generate page number buttons
        this.generatePageButtons(pagination.page, pagination.totalPages);
        
        // Update results info
        this.updateResultsInfo(pagination);
        
        // Update button states
        document.getElementById('prevPageBtn').disabled = !pagination.hasPrev;
        document.getElementById('nextPageBtn').disabled = !pagination.hasNext;
        document.getElementById('prevPageBtnBottom').disabled = !pagination.hasPrev;
        document.getElementById('nextPageBtnBottom').disabled = !pagination.hasNext;
    }

    generatePageButtons(currentPage, totalPages) {
        const pageNumbers = document.getElementById('pageNumbers');
        const pageNumbersBottom = document.getElementById('pageNumbersBottom');
        
        // Clear existing buttons
        pageNumbers.innerHTML = '';
        pageNumbersBottom.innerHTML = '';
        
        if (totalPages <= 1) {
            return; // Don't show page numbers if there's only one page
        }
        
        const buttonConfigs = this.createPageButtonConfigs(currentPage, totalPages);
        
        // Create buttons for top pagination
        buttonConfigs.forEach(config => {
            const button = this.createPageButton(config.pageNumber, config.isActive, config.isEllipsis);
            pageNumbers.appendChild(button);
        });
        
        // Create buttons for bottom pagination
        buttonConfigs.forEach(config => {
            const button = this.createPageButton(config.pageNumber, config.isActive, config.isEllipsis);
            pageNumbersBottom.appendChild(button);
        });
    }

    createPageButtonConfigs(currentPage, totalPages) {
        const configs = [];
        const maxVisiblePages = 7; // Show up to 7 page buttons
        
        if (totalPages <= maxVisiblePages) {
            // Show all pages if total is small
            for (let i = 1; i <= totalPages; i++) {
                configs.push({ pageNumber: i, isActive: i === currentPage, isEllipsis: false });
            }
        } else {
            // Smart pagination with ellipsis
            const showEllipsis = totalPages > maxVisiblePages;
            
            if (currentPage <= 4) {
                // Show first pages: 1 2 3 4 5 ... last
                for (let i = 1; i <= 5; i++) {
                    configs.push({ pageNumber: i, isActive: i === currentPage, isEllipsis: false });
                }
                if (showEllipsis) {
                    configs.push({ pageNumber: null, isActive: false, isEllipsis: true });
                }
                configs.push({ pageNumber: totalPages, isActive: false, isEllipsis: false });
            } else if (currentPage >= totalPages - 3) {
                // Show last pages: 1 ... (last-4) (last-3) (last-2) (last-1) last
                configs.push({ pageNumber: 1, isActive: false, isEllipsis: false });
                if (showEllipsis) {
                    configs.push({ pageNumber: null, isActive: false, isEllipsis: true });
                }
                for (let i = totalPages - 4; i <= totalPages; i++) {
                    configs.push({ pageNumber: i, isActive: i === currentPage, isEllipsis: false });
                }
            } else {
                // Show middle pages: 1 ... (current-1) current (current+1) ... last
                configs.push({ pageNumber: 1, isActive: false, isEllipsis: false });
                if (showEllipsis) {
                    configs.push({ pageNumber: null, isActive: false, isEllipsis: true });
                }
                for (let i = currentPage - 1; i <= currentPage + 1; i++) {
                    configs.push({ pageNumber: i, isActive: i === currentPage, isEllipsis: false });
                }
                if (showEllipsis) {
                    configs.push({ pageNumber: null, isActive: false, isEllipsis: true });
                }
                configs.push({ pageNumber: totalPages, isActive: false, isEllipsis: false });
            }
        }
        
        return configs;
    }

    createPageButton(pageNumber, isActive, isEllipsis = false) {
        if (isEllipsis) {
            const ellipsis = document.createElement('div');
            ellipsis.className = 'page-ellipsis';
            ellipsis.textContent = '...';
            return ellipsis;
        }
        
        const button = document.createElement('button');
        button.className = `page-number-btn ${isActive ? 'active' : ''}`;
        button.textContent = pageNumber;
        button.addEventListener('click', () => {
            if (pageNumber !== this.currentPage) {
                this.currentPage = pageNumber;
                this.updateUrlParams({ page: this.currentPage });
                this.loadLogs(`Navigating to page ${pageNumber}...`);
                
                // Scroll to top of log content
                const runContainer = document.getElementById('runContainer');
                if (runContainer) {
                    runContainer.scrollIntoView({ behavior: 'smooth', block: 'start' });
                }
            }
        });
        return button;
    }

    updateResultsInfo(pagination) {
        const itemsPerPage = parseInt(document.getElementById('itemsPerPage').value) || 25;
        const startItem = (pagination.page - 1) * itemsPerPage + 1;
        const endItem = Math.min(pagination.page * itemsPerPage, pagination.total);
        
        const resultsText = `Results: ${startItem} - ${endItem} of ${pagination.total}`;
        
        // Update both results info elements
        document.getElementById('resultsInfo').textContent = resultsText;
        document.getElementById('resultsInfoBottom').textContent = resultsText;
    }

    previousPage() {
        if (this.currentPage > 1) {
            this.currentPage--;
            this.updateUrlParams({ page: this.currentPage });
            this.loadLogs('Navigating to previous page...');
        }
    }

    nextPage() {
        this.currentPage++;
        this.updateUrlParams({ page: this.currentPage });
        this.loadLogs('Navigating to next page...');
    }

    async loadStats() {
        if (!this.selectedRunId) return;

        try {
            const response = await fetch(`/api/stats/${this.selectedRunId}`);
            const data = await response.json();
            
            this.renderStats(data.stats);
            document.getElementById('statsSection').style.display = 'block';
            
        } catch (error) {
            console.error('Error loading stats:', error);
        }
    }

    renderStats(stats) {
        const container = document.getElementById('statsContent');
        
        let html = '<div class="stats-grid">';
        
        html += `
            <div class="stat-item">
                <div class="stat-label">Total Logs</div>
                <div class="stat-value">${Object.values(stats.levels).reduce((a, b) => a + b, 0)}</div>
            </div>
            <div class="stat-item">
                <div class="stat-label">Panic Logs</div>
                <div class="stat-value">${stats.panicCount}</div>
            </div>
            <div class="stat-item">
                <div class="stat-label">Log Levels</div>
                <div class="stat-value">${Object.keys(stats.levels).length}</div>
            </div>
            <div class="stat-item">
                <div class="stat-label">Targets</div>
                <div class="stat-value">${Object.keys(stats.targets).length}</div>
            </div>
        `;
        
        // Add task statistics
        if (stats.tasks && Object.keys(stats.tasks).length > 0) {
            html += `
                <div class="stat-item">
                    <div class="stat-label">Unique Tasks</div>
                    <div class="stat-value">${Object.keys(stats.tasks).length - 1}</div>
                </div>
            `;
        }
        
        html += '</div>';
        container.innerHTML = html;
    }

    showUploadModal() {
        document.getElementById('uploadModal').classList.add('active');
    }

    async uploadFiles() {
        const logFileInput = document.getElementById('logFileInput');
        const resultFileInput = document.getElementById('resultFileInput');
        
        if (!logFileInput.files[0]) {
            this.showError('Log file is required');
            return;
        }

        if (!resultFileInput.files[0]) {
            this.showError('Result file is required for a complete Ruse run');
            return;
        }

        const formData = new FormData();
        formData.append('logFile', logFileInput.files[0]);
        formData.append('resultFile', resultFileInput.files[0]);

        const progressContainer = document.getElementById('uploadProgress');
        const progressFill = progressContainer.querySelector('.progress-fill');
        
        progressContainer.style.display = 'block';
        progressFill.style.width = '0%';

        try {
            // Simulate progress for UI feedback
            const progressInterval = setInterval(() => {
                const currentWidth = parseInt(progressFill.style.width) || 0;
                if (currentWidth < 90) {
                    progressFill.style.width = `${currentWidth + 10}%`;
                }
            }, 200);

            const response = await fetch('/api/upload', {
                method: 'POST',
                body: formData
            });

            clearInterval(progressInterval);
            progressFill.style.width = '100%';

            if (!response.ok) {
                throw new Error('Upload failed');
            }

            const result = await response.json();
            
            setTimeout(() => {
                document.getElementById('uploadModal').classList.remove('active');
                progressContainer.style.display = 'none';
                
                // Clear file inputs
                logFileInput.value = '';
                resultFileInput.value = '';
                
                this.showSuccess('Ruse run uploaded and processed successfully!');
                
                // Refresh the runs list
                this.loadRuseRuns();
            }, 500);

        } catch (error) {
            console.error('Upload error:', error);
            progressContainer.style.display = 'none';
            this.showError('Failed to upload file(s)');
        }
    }

    showLoading(message) {
        // You could implement a loading overlay here
        console.log('Loading:', message);
    }

    showSuccess(message) {
        // Simple notification - you could enhance this
        alert(message);
    }

    showError(message) {
        // Simple notification - you could enhance this
        alert('Error: ' + message);
    }

    showLogsLoading(message = 'Loading logs...') {
        // First ensure the logs container is visible
        const logsContainer = document.getElementById('logsContainer');
        if (logsContainer) {
            logsContainer.style.display = 'block';
        }
        
        const overlay = document.getElementById('logsLoadingOverlay');
        const text = document.getElementById('logsLoadingText');
        
        text.textContent = message;
        overlay.classList.remove('hidden');
    }

    hideLogsLoading() {
        const overlay = document.getElementById('logsLoadingOverlay');
        overlay.classList.add('hidden');
    }

    // showSimpleLoading(message) {
    //     // Fallback loading display
    //     const logsContent = document.getElementById('logsContent');
    //     if (logsContent) {
    //         logsContent.innerHTML = `<div style="display: flex; justify-content: center; align-items: center; height: 200px; font-size: 1.2rem; color: #667eea;">
    //             <div style="text-align: center;">
    //                 <div style="margin-bottom: 1rem;">⟲</div>
    //                 <div>${message}</div>
    //             </div>
    //         </div>`;
    //     }
    // }

    // hideSimpleLoading() {
    //     // This will be cleared when renderLogs is called
    // }

    getFileNameFromPath(path) {
        return path.split('/').pop() || path;
    }

    // URL parameter management
    updateUrlParams(params) {
        const url = new URL(window.location);
        Object.entries(params).forEach(([key, value]) => {
            if (value !== null && value !== undefined && value !== '') {
                url.searchParams.set(key, value);
            } else {
                url.searchParams.delete(key);
            }
        });
        window.history.pushState({}, '', url);
    }

    getUrlParams() {
        const params = new URLSearchParams(window.location.search);
        return {
            run: params.get('run'),
            tab: params.get('tab'),
            page: parseInt(params.get('page')) || 1,
            expanded: params.get('expanded')?.split(',').filter(Boolean) || []
        };
    }

    restoreStateFromUrl() {
        const params = this.getUrlParams();
        
        // Restore selected run if specified
        if (params.run) {
            this.selectedRunId = params.run;
        }
        
        // Restore page if specified
        if (params.page) {
            this.currentPage = params.page;
        }
        
        // Note: expanded logs will be restored after logs are loaded
        return params;
    }
}

// Initialize the application when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.app = new LogViewer();
});

