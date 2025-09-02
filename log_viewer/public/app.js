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
        
        // Bind "All" checkbox events
        this.bindFilterAllCheckboxes();
        
        // Pagination
        document.getElementById('prevPageBtn').addEventListener('click', () => this.previousPage());
        document.getElementById('nextPageBtn').addEventListener('click', () => this.nextPage());
        document.getElementById('prevPageBtnBottom').addEventListener('click', () => this.previousPage());
        document.getElementById('nextPageBtnBottom').addEventListener('click', () => this.nextPage());

        // Upload modal
        this.bindUploadEvents();
        
        // Modal close events
        document.querySelectorAll('.modal-close').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.target.closest('.modal').classList.remove('active');
            });
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
                            <span class="info-value">${results.sysinfo?.name || 'Unknown'} ${results.sysinfo?.os || ''}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">Kernel:</span>
                            <span class="info-value">${results.sysinfo?.kernel || 'Unknown'}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">CPU:</span>
                            <span class="info-value">${results.sysinfo?.cpu || 'Unknown'}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">Cores:</span>
                            <span class="info-value">${results.sysinfo?.cpu_core_count || 'Unknown'}</span>
                        </div>
                    </div>
                </div>
                
                <div class="config-section">
                    <h4>Configuration</h4>
                    <div class="config-summary">
                        <div class="config-item">
                            <span class="config-label">Timeout:</span>
                            <span class="config-value">${results.config?.timeout || 'Unknown'}s</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Max Iterations:</span>
                            <span class="config-value">${results.config?.max_iterations || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Multi-thread:</span>
                            <span class="config-value">${results.config?.multi_thread ? 'Yes' : 'No'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Max Context Depth:</span>
                            <span class="config-value">${results.config?.max_context_depth || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Iteration Workers:</span>
                            <span class="config-value">${results.config?.iteration_workers_count || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Max Task Memory:</span>
                            <span class="config-value">${results.config?.max_task_mem || 'Unknown'}</span>
                        </div>
                        <div class="config-item">
                            <span class="config-label">Bank Type:</span>
                            <span class="config-value">${results.config?.bank_type || 'Unknown'}</span>
                        </div>
                    </div>
                                        
                    <button class="config-expand-btn">▼ Show Details</button>
                    <div class="config-details" style="display: none;">
                        <h5>Full Configuration</h5>
                        <div class="full-config">
                            <pre class="config-json">${JSON.stringify(results.config, null, 2)}</pre>
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
                                        <span>Max Size: ${task.total_statistics.MaxSize}</span>
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
                                                <span>Max Size: ${iter.statistics.MaxSize}</span>
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
            
            const params = new URLSearchParams({
                page: this.currentPage,
                limit: 100,
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
        const logsCount = document.getElementById('logsCount');
        const fileName = document.getElementById('currentFileName');

        logsCount.textContent = `${data.pagination.total} logs`;
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
                    <div class="log-header-message">${this.escapeHtml(message)}</div>
                    ${this.renderTaskInfo(log)}
                </div>
                <div class="log-body" id="log-body-${index}">
                    <div class="log-message">${this.escapeHtml(message)}</div>
                    ${this.renderLogFields(log)}
                    ${this.renderExtensions(log._meta.extensions, index)}
                    ${isPanic ? this.renderPanicInfo(log._meta) : ''}
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
                    <span class="task-name">${this.escapeHtml(log._meta.span.task)}</span>
                    ${(log._meta.span.iteration !== undefined) ? `<span class="task-iteration">(iteration: ${log._meta.span.iteration})</span>` : ''}
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

    renderLogFields(log) {
        if (!log.fields) return '';

        const fieldsToSkip = ['message', 'panic.backtrace', 'panic.location'];
        const fields = Object.entries(log.fields)
            .filter(([key]) => !fieldsToSkip.includes(key))
            .filter(([key]) => !key.includes('.mermaid') && !key.includes('.json')); // Skip extensions

        if (fields.length === 0) return '';

        let html = '<div class="log-fields">';
        fields.forEach(([key, value]) => {
            html += `
                <div class="log-field">
                    <div class="log-field-key">${this.escapeHtml(key)}:</div>
                    <div class="log-field-value">${this.escapeHtml(String(value))}</div>
                </div>
            `;
        });
        html += '</div>';

        return html;
    }

    renderExtensions(extensions, logIndex = 0, isNested = false) {
        if (Object.keys(extensions).length === 0) return '';

        // First, collect all extensions (including nested ones from JSON)
        // const allExtensions = this.flattenExtensions(extensions);

        let html = '';
        
        // Handle Mermaid diagrams (including those from nested JSON)
        if (extensions.mermaid) {
            Object.entries(extensions.mermaid).forEach(([key, diagram], mermaidIndex) => {
                const mermaidId = `mermaid-log-${logIndex}-${mermaidIndex}-${Math.random().toString(36).substr(2, 9)}`;
                html += `
                    <div class="extension-content">
                        <div class="extension-title">
                            📊 Mermaid Diagram: ${key}
                            <button class="mermaid-toggle-btn" onclick="app.toggleMermaidRaw('${mermaidId}')" title="Toggle raw text">
                                <span id="toggle-icon-${mermaidId}">👁️</span>
                            </button>
                        </div>
                        <div class="mermaid-diagram" id="diagram-${mermaidId}">
                            <pre class="mermaid">${this.escapeHtml(diagram)}</pre>
                        </div>
                        <div class="mermaid-raw" id="raw-${mermaidId}" style="display: none;">
                            <pre class="mermaid-raw-text">${this.escapeHtml(diagram)}</pre>
                        </div>
                    </div>
                `;
            });
        }

        // Handle JSON extensions  
        if (extensions.json) {
            Object.entries(extensions.json).forEach(([key, jsonData], jsonIndex) => {
                const jsonId = `json-log-${logIndex}-${jsonIndex}-${Math.random().toString(36).substr(2, 9)}`;
                html += `
                    <div class="extension-content">
                        <div class="extension-title json-toggle" onclick="app.toggleJsonSection('${jsonId}')">
                            <span class="json-toggle-icon" id="icon-${jsonId}">▶</span>
                            📄 JSON Data: ${key}
                        </div>
                        <div class="json-content json-collapsible" id="${jsonId}" style="display: none;">
                `;
                
                if (jsonData && typeof jsonData === 'object' && jsonData.raw) {
                    // Render JSON fields 
                    html += '<div class="log-fields">';
                    html += this.renderJsonFields(jsonData.raw, '', jsonId);
                    
                    // Render any nested extensions within this JSON
                    if (jsonData.extensions && Object.keys(jsonData.extensions).length > 0) {
                        html += this.renderExtensions(jsonData.extensions, 0, true);
                    }
                    html += '</div>';
                } else {
                    // Handle simple JSON value
                    html += `<pre class="json-display">${this.escapeHtml(JSON.stringify(jsonData, null, 2))}</pre>`;
                }
                
                html += '</div></div>';
            });
        }

        // Handle other extensions
        // Object.entries(allExtensions).forEach(([extType, extData]) => {
        //     if (extType === 'mermaid' || extType === 'json') return; // Already handled

        //     html += `
        //         <div class="extension-content">
        //             <div class="extension-title">🔧 Extension: ${extType}</div>
        //             <div class="log-fields">
        //     `;
            
        //     Object.entries(extData).forEach(([key, value]) => {
        //         html += `
        //             <div class="log-field">
        //                 <div class="log-field-key">${this.escapeHtml(key)}:</div>
        //                 <div class="log-field-value">${this.escapeHtml(String(value))}</div>
        //             </div>
        //         `;
        //     });
            
        //     html += '</div></div>';
        // });

        return html;
    }

    /**
     * Flatten all extensions but keep JSON-nested extensions separate for inline rendering
     */
    flattenExtensions(extensions, result = {}) {
        for (const [extType, extData] of Object.entries(extensions)) {
            if (extType === 'json') {
                // For JSON extensions, do NOT merge nested extensions 
                // (they will be rendered inline within JSON sections)
                // Just skip processing nested extensions here
            } else {
                // For non-JSON extensions, add them directly
                if (!result[extType]) {
                    result[extType] = {};
                }
                Object.assign(result[extType], extData);
            }
        }
        return result;
    }



    renderJsonFields(obj, keyPrefix = '', jsonId) {
        let html = '';
        
        if (typeof obj !== 'object' || obj === null) {
            return `<div class="log-field">
                        <div class="log-field-key">${this.escapeHtml(keyPrefix)}:</div>
                        <div class="log-field-value">${this.escapeHtml(String(obj))}</div>
                    </div>`;
        }
        
        for (const [key, value] of Object.entries(obj)) {
            const fullKey = keyPrefix ? `${keyPrefix}.${key}` : key;
            
            if (typeof value === 'object' && value !== null) {
                // Recursively render nested objects
                const recursiveJsonId = `${jsonId}-${fullKey}`;
                html += `
                    <div class="json-toggle" onclick="app.toggleJsonSection('${recursiveJsonId}')">
                        <span class="json-toggle-icon" id="icon-${recursiveJsonId}">▶</span>
                        ${key}
                    </div>
                    <div class="json-content json-collapsible" id="${recursiveJsonId}" style="display: none;">
                `;
                html += this.renderJsonFields(value, fullKey, jsonId);
                html += '</div>';
            } else {
                // Skip extension fields (they'll be handled by renderExtensions)
                if (fullKey.endsWith('.mermaid') || fullKey.endsWith('.json') || 
                    fullKey.includes('.mermaid.') || fullKey.includes('.json.')) {
                    // Don't render extension fields as regular fields
                    // They will be handled by the extension rendering system
                } else {
                    // Render as regular field
                    const displayValue = Array.isArray(value) ? JSON.stringify(value) : String(value);
                    html += `
                        <div class="log-field">
                            <div class="log-field-key">${this.escapeHtml(key)}:</div>
                            <div class="log-field-value">${this.escapeHtml(displayValue)}</div>
                        </div>
                    `;
                }
            }
        }
        
        return html;
    }

    renderPanicInfo(meta) {
        if (!meta.isPanic) return '';

        let html = `
            <div class="panic-info">
                <div class="panic-title">🚨 Panic Information</div>
        `;

        if (meta.backtrace) {
            if (meta.backtrace.type === 'disabled') {
                html += '<p>Backtrace collection is disabled</p>';
            } else {
                html += '<div class="extension-title">Stack Trace:</div>';
                if (meta.backtrace.frames.length > 0) {
                    const backtrace = meta.backtrace.frames
                        .map(frame => frame.raw || `${frame.symbol} at ${frame.location}`)
                        .join('\n');
                    html += `<div class="backtrace">${this.escapeHtml(backtrace)}</div>`;
                } else {
                    html += '<p>No meaningful stack frames found</p>';
                }
            }
        }

        html += '</div>';
        return html;
    }

    toggleLogEntry(index) {
        const body = document.getElementById(`log-body-${index}`);
        body.classList.toggle('expanded');
        
        // Re-run Mermaid for newly expanded entries
        if (body.classList.contains('expanded')) {
            // Only render Mermaid diagrams in this specific expanded log entry
            setTimeout(() => this.triggerMermaidRendering(body), 100);
        }
    }

    async triggerMermaidRendering(container = null) {
        try {
            // Wait for mermaid to be ready
            if (typeof mermaid === 'undefined' || typeof window.mermaid === 'undefined') {
                console.warn('Mermaid is not loaded yet, retrying...');
                setTimeout(() => this.triggerMermaidRendering(container), 500);
                return;
            }

            console.log('Triggering Mermaid rendering for specific container...');
            
            // Find mermaid elements only in visible/expanded areas
            let mermaidElements;
            
            if (container) {
                // Render only truly visible diagrams in the specific container
                const allDiagrams = container.querySelectorAll('.mermaid:not([data-processed])');
                mermaidElements = Array.from(allDiagrams).filter(diagram => this.isDiagramVisible(diagram));
                console.log(`Found ${mermaidElements.length} visible unprocessed diagrams in container`);
            } else {
                // Fallback: find all truly visible mermaid elements
                const allDiagrams = document.querySelectorAll('.mermaid:not([data-processed])');
                mermaidElements = Array.from(allDiagrams).filter(diagram => this.isDiagramVisible(diagram));
                console.log(`Found ${mermaidElements.length} visible unprocessed diagrams on page`);
            }
            
            if (mermaidElements.length === 0) {
                console.log('No unprocessed mermaid elements found in target area');
                return;
            }
            
            // Use mermaid.run() with specific nodes for targeted rendering
            await window.mermaid.run({
                nodes: mermaidElements
            });
            
            console.log('Mermaid rendering completed');
            
        } catch (error) {
            console.error('Error in triggerMermaidRendering:', error);
        }
    }

    isDiagramVisible(diagram) {
        // Check if the diagram is truly visible by checking all parent containers
        
        // 1. Check if the diagram is in an expanded log entry
        const logBody = diagram.closest('.log-body');
        if (logBody && !logBody.classList.contains('expanded')) {
            return false; // Log entry is collapsed
        }
        
        // 2. Check if the diagram is inside any collapsed JSON sections
        let currentElement = diagram;
        while (currentElement && currentElement !== document.body) {
            // Check if this element is a collapsed JSON section
            if (currentElement.classList && currentElement.classList.contains('json-collapsible')) {
                const style = window.getComputedStyle(currentElement);
                if (style.display === 'none' || currentElement.style.display === 'none') {
                    return false; // JSON section is collapsed
                }
            }
            currentElement = currentElement.parentElement;
        }
        
        // 3. Check if the diagram is inside a hidden mermaid-raw div (when showing raw text)
        const mermaidRaw = diagram.closest('.mermaid-raw');
        if (mermaidRaw) {
            const style = window.getComputedStyle(mermaidRaw);
            if (style.display === 'none' || mermaidRaw.style.display === 'none') {
                return false; // Raw text view is hidden
            }
        }
        
        return true; // Diagram is truly visible
    }

    toggleJsonSection(jsonId) {
        const section = document.getElementById(jsonId);
        const icon = document.getElementById(`icon-${jsonId}`);
        
        if (section && icon) {
            if (section.style.display === 'none') {
                section.style.display = 'block';
                icon.textContent = '▼';
                // Trigger Mermaid rendering only for this specific JSON section
                setTimeout(() => this.triggerMermaidRendering(section), 100);
            } else {
                section.style.display = 'none';
                icon.textContent = '▶';
            }
        }
    }

    toggleMermaidRaw(mermaidId) {
        const diagramDiv = document.getElementById(`diagram-${mermaidId}`);
        const rawDiv = document.getElementById(`raw-${mermaidId}`);
        const toggleIcon = document.getElementById(`toggle-icon-${mermaidId}`);
        
        if (diagramDiv && rawDiv && toggleIcon) {
            if (rawDiv.style.display === 'none') {
                // Show raw text, hide diagram
                diagramDiv.style.display = 'none';
                rawDiv.style.display = 'block';
                toggleIcon.textContent = '📊';
                toggleIcon.parentElement.title = 'Show diagram';
            } else {
                // Show diagram, hide raw text
                diagramDiv.style.display = 'block';
                rawDiv.style.display = 'none';
                toggleIcon.textContent = '👁️';
                toggleIcon.parentElement.title = 'Toggle raw text';
                // Trigger Mermaid rendering when switching back to diagram
                setTimeout(() => this.triggerMermaidRendering(diagramDiv), 100);
            }
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
        if (search) filters.search = search;

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
        // Update page info
        document.getElementById('pageInfo').textContent = 
            `Page ${pagination.page} of ${pagination.totalPages}`;
        document.getElementById('pageInfoBottom').textContent = 
            `Page ${pagination.page} of ${pagination.totalPages}`;

        // Update button states
        document.getElementById('prevPageBtn').disabled = !pagination.hasPrev;
        document.getElementById('nextPageBtn').disabled = !pagination.hasNext;
        document.getElementById('prevPageBtnBottom').disabled = !pagination.hasPrev;
        document.getElementById('nextPageBtnBottom').disabled = !pagination.hasNext;
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

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
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
