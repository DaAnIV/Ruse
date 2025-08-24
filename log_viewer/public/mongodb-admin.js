class MongoDBAdmin {
    constructor() {
        this.status = null;
        this.collections = [];
        this.selectedCollection = null;
        
        this.bindEvents();
        this.loadInitialData();
    }

    bindEvents() {
        // Quick action buttons
        document.getElementById('refreshBtn').addEventListener('click', () => this.loadInitialData());
        document.getElementById('optimizeBtn').addEventListener('click', () => this.optimizeDatabase());
        document.getElementById('exportBtn').addEventListener('click', () => this.exportStats());

        // Danger zone buttons
        document.getElementById('clearAllBtn').addEventListener('click', () => this.confirmClearAll());
        document.getElementById('resetIndexesBtn').addEventListener('click', () => this.confirmResetIndexes());

        // Collection dropdown
        document.getElementById('collectionDropdown').addEventListener('change', (e) => {
            this.loadCollectionDetails(e.target.value);
        });

        // Confirmation dialog
        document.getElementById('confirmNo').addEventListener('click', () => this.hideConfirmation());
        document.getElementById('confirmYes').addEventListener('click', () => this.executeConfirmedAction());
    }

    async loadInitialData() {
        this.showLoading('Refreshing data...');
        
        try {
            await Promise.all([
                this.loadConnectionStatus(),
                this.loadCollections()
            ]);
            this.populateCollectionDropdown();
            this.hideLoading();
        } catch (error) {
            console.error('Error loading initial data:', error);
            this.showError('Failed to load data: ' + error.message);
            this.hideLoading();
        }
    }

    async loadConnectionStatus() {
        try {
            const response = await fetch('/api/mongodb/status');
            
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }
            
            this.status = await response.json();
            
            const statusElement = document.getElementById('connectionStatus');
            const indicatorElement = document.getElementById('statusIndicator');
            const contentElement = document.getElementById('statusContent');
            
            if (this.status.available) {
                indicatorElement.className = 'status-indicator status-online';
                contentElement.innerHTML = `
                    <div class="stat-grid">
                        <div class="stat-item">
                            <div class="stat-value">✅</div>
                            <div class="stat-label">Connected</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">${this.status.stats?.totalCaches || 0}</div>
                            <div class="stat-label">Cache Collections</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">${this.status.stats?.totalCollections || 0}</div>
                            <div class="stat-label">Log Collections</div>
                        </div>
                    </div>
                    <p style="margin-top: 1rem; font-size: 0.875rem; color: #6c757d;">
                        Database: ${this.status.database || 'log_viewer'}<br>
                        Connection: ${this.status.connection || 'mongodb://localhost:27017'}
                    </p>
                `;
                
                this.loadDatabaseStats();
            } else {
                indicatorElement.className = 'status-indicator status-offline';
                contentElement.innerHTML = `
                    <div class="error">
                        <strong>MongoDB Unavailable</strong><br>
                        ${this.status.message || 'Could not connect to MongoDB'}
                    </div>
                    <p style="margin-top: 1rem; font-size: 0.875rem;">
                        Make sure MongoDB is running on localhost:27017
                    </p>
                `;
            }
        } catch (error) {
            console.error('Error loading status:', error);
            const indicatorElement = document.getElementById('statusIndicator');
            const contentElement = document.getElementById('statusContent');
            
            indicatorElement.className = 'status-indicator status-offline';
            contentElement.innerHTML = `<div class="error">Error: ${error.message}</div>`;
        }
    }

    async loadDatabaseStats() {
        const dbStatsElement = document.getElementById('dbStats');
        const storageStatsElement = document.getElementById('storageStats');
        
        if (this.status && this.status.available && this.status.stats) {
            const stats = this.status.stats;
            
            dbStatsElement.innerHTML = `
                <div class="stat-grid">
                    <div class="stat-item">
                        <div class="stat-value">${stats.objects || 0}</div>
                        <div class="stat-label">Total Documents</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value">${stats.totalCollections || 0}</div>
                        <div class="stat-label">Collections</div>
                    </div>
                </div>
            `;
            
            storageStatsElement.innerHTML = `
                <div class="stat-grid">
                    <div class="stat-item">
                        <div class="stat-value size-display">${this.formatBytes(stats.dataSize || 0)}</div>
                        <div class="stat-label">Data Size</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value size-display">${this.formatBytes(stats.storageSize || 0)}</div>
                        <div class="stat-label">Storage Size</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value size-display">${this.formatBytes(stats.indexSize || 0)}</div>
                        <div class="stat-label">Index Size</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value size-display">${this.formatBytes(stats.totalSize || 0)}</div>
                        <div class="stat-label">Total Size</div>
                    </div>
                </div>
                ${stats.avgObjSize ? `<p style="margin-top: 1rem; font-size: 0.875rem; color: #6c757d;">Average object size: ${this.formatBytes(stats.avgObjSize)}</p>` : ''}
            `;
        } else {
            // Show unavailable state when MongoDB is not accessible
            dbStatsElement.innerHTML = `
                <div class="error">
                    Database statistics not available
                </div>
            `;
            
            storageStatsElement.innerHTML = `
                <div class="error">
                    Storage statistics not available
                </div>
            `;
        }
    }

    async loadCollections() {
        try {
            const response = await fetch('/api/mongodb/collections');
            
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }
            
            const data = await response.json();
            this.collections = data.collections || [];
            
            this.renderCollectionsTable();
        } catch (error) {
            console.error('Error loading collections:', error);
            document.getElementById('collectionsContent').innerHTML = 
                `<div class="error">Failed to load collections: ${error.message}</div>`;
        }
    }

    renderCollectionsTable() {
        const container = document.getElementById('collectionsContent');
        
        if (this.collections.length === 0) {
            container.innerHTML = '<p>No collections found.</p>';
            return;
        }
        
        let html = `
            <div class="collections-table-container">
                <table class="collections-table">
                    <thead>
                        <tr>
                            <th>Collection Name</th>
                            <th>Type</th>
                            <th>Documents</th>
                            <th>Size</th>
                            <th>Storage</th>
                            <th>Indexes</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
        `;
        
        this.collections.forEach(collection => {
            html += `
                <tr>
                    <td>
                        <strong>${collection.name}</strong>
                        ${collection.cacheHash ? `<br><small style="color: #6c757d;">Hash: ${collection.cacheHash.substring(0, 8)}...</small>` : ''}
                    </td>
                    <td>
                        <span class="collection-type collection-${collection.type}">
                            ${collection.type}
                        </span>
                    </td>
                    <td>${collection.count?.toLocaleString("en-GB") || 'N/A'}</td>
                    <td class="size-display">${collection.size ? this.formatBytes(collection.size) : 'N/A'}</td>
                    <td class="size-display">${collection.storageSize ? this.formatBytes(collection.storageSize) : 'N/A'}</td>
                    <td>${collection.indexes || 0}</td>
                    <td>
                        ${collection.type === 'logs' ? 
                            `<button onclick="mongoAdmin.deleteCollection('${collection.cacheHash}')" class="btn-danger" style="font-size: 0.75rem; padding: 0.25rem 0.5rem;">Delete</button>` : 
                            '<span style="color: #6c757d;">—</span>'
                        }
                    </td>
                </tr>
            `;
        });
        
        html += '</tbody></table></div>';
        container.innerHTML = html;
    }

    populateCollectionDropdown() {
        const dropdown = document.getElementById('collectionDropdown');
        dropdown.innerHTML = '<option value="">Select a collection...</option>';
        
        this.collections.forEach(collection => {
            if (collection.type === 'logs') {
                const option = document.createElement('option');
                option.value = collection.cacheHash;
                option.textContent = `${collection.name} (${collection.count?.toLocaleString("en-GB") || 0} logs)`;
                dropdown.appendChild(option);
            }
        });
    }

    async loadCollectionDetails(cacheHash) {
        const container = document.getElementById('collectionDetails');
        
        if (!cacheHash) {
            container.innerHTML = '';
            return;
        }
        
        container.innerHTML = '<div class="loading">Loading collection details...</div>';
        
        try {
            const response = await fetch(`/api/mongodb/collection/${cacheHash}`);
            const data = await response.json();
            
            this.renderCollectionDetails(data);
        } catch (error) {
            console.error('Error loading collection details:', error);
            container.innerHTML = `<div class="error">Failed to load details: ${error.message}</div>`;
        }
    }

    renderCollectionDetails(data) {
        const container = document.getElementById('collectionDetails');
        
        let html = `
            <div style="border: 1px solid #e9ecef; border-radius: 8px; padding: 1rem; margin-top: 1rem;">
                <h4>${data.collectionName}</h4>
                
                <div class="stat-grid" style="margin: 1rem 0;">
                    <div class="stat-item">
                        <div class="stat-value">${data.count.toLocaleString("en-GB")}</div>
                        <div class="stat-label">Total Logs</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value size-display">${this.formatBytes(data.stats.size)}</div>
                        <div class="stat-label">Data Size</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value size-display">${this.formatBytes(data.stats.storageSize)}</div>
                        <div class="stat-label">Storage Size</div>
                    </div>
                    <div class="stat-item">
                        <div class="stat-value">${data.distribution.panicCount}</div>
                        <div class="stat-label">Panic Logs</div>
                    </div>
                </div>
                
                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin: 1rem 0;">
                    <div>
                        <h5>Log Levels</h5>
                        <ul style="margin: 0.5rem 0;">
        `;
        
        Object.entries(data.distribution.levels).forEach(([level, count]) => {
            html += `<li><strong>${level}:</strong> ${count.toLocaleString("en-GB")}</li>`;
        });
        
        html += `
                        </ul>
                    </div>
                    <div>
                        <h5>Top Targets</h5>
                        <ul style="margin: 0.5rem 0;">
        `;
        
        Object.entries(data.distribution.targets).slice(0, 5).forEach(([target, count]) => {
            html += `<li><strong>${target}:</strong> ${count.toLocaleString("en-GB")}</li>`;
        });
        
        html += `
                        </ul>
                    </div>
                </div>
        `;
        
        if (data.timeRange) {
            html += `
                <p style="margin: 1rem 0; font-size: 0.875rem; color: #6c757d;">
                    <strong>Time Range:</strong> 
                    ${new Date(data.timeRange.start).toLocaleString("en-GB")} to 
                    ${new Date(data.timeRange.end).toLocaleString("en-GB")}
                </p>
            `;
        }
        
        html += `
                <div class="action-buttons" style="margin-top: 1rem;">
                    <button onclick="mongoAdmin.deleteCollection('${data.cacheHash}')" class="btn-danger">
                        🗑️ Delete Collection
                    </button>
                    <button onclick="mongoAdmin.deleteSpecificLogs('${data.cacheHash}')" class="btn-warning">
                        🎯 Delete Specific Logs
                    </button>
                </div>
            </div>
        `;
        
        container.innerHTML = html;
    }

    async deleteCollection(cacheHash) {
        const collection = this.collections.find(c => c.cacheHash === cacheHash);
        const collectionName = collection ? collection.name : cacheHash;
        
        this.showConfirmation(
            'Delete Collection',
            `Are you sure you want to delete the collection "${collectionName}"? This will permanently remove all logs in this collection.`,
            () => this.executeDeleteCollection(cacheHash)
        );
    }

    async executeDeleteCollection(cacheHash) {
        try {
            this.showLoading('Deleting collection...');
            
            const response = await fetch(`/api/mongodb/collection/${cacheHash}`, {
                method: 'DELETE'
            });
            
            const result = await response.json();
            
            if (response.ok) {
                this.showSuccess(result.message);
                await this.loadInitialData();
                document.getElementById('collectionDropdown').value = '';
                document.getElementById('collectionDetails').innerHTML = '';
            } else {
                this.showError('Failed to delete collection: ' + result.error);
            }
        } catch (error) {
            console.error('Error deleting collection:', error);
            this.showError('Failed to delete collection: ' + error.message);
        }
    }

    async deleteSpecificLogs(cacheHash) {
        // For now, show a simple prompt. This could be enhanced with a modal form
        const criteria = prompt('Enter deletion criteria (JSON format):\nExample: {"level": "DEBUG"} or {"target": "app::test"}');
        
        if (criteria) {
            try {
                const filters = JSON.parse(criteria);
                
                this.showConfirmation(
                    'Delete Specific Logs',
                    `Are you sure you want to delete logs matching: ${criteria}?`,
                    () => this.executeDeleteSpecificLogs(cacheHash, filters)
                );
            } catch (error) {
                this.showError('Invalid JSON format: ' + error.message);
            }
        }
    }

    async executeDeleteSpecificLogs(cacheHash, filters) {
        try {
            this.showLoading('Deleting specific logs...');
            
            const response = await fetch(`/api/mongodb/logs/${cacheHash}`, {
                method: 'DELETE',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ filters })
            });
            
            const result = await response.json();
            
            if (response.ok) {
                this.showSuccess(`${result.message} (${result.deletedCount} logs deleted)`);
                await this.loadCollectionDetails(cacheHash);
                await this.loadCollections();
            } else {
                this.showError('Failed to delete logs: ' + result.error);
            }
        } catch (error) {
            console.error('Error deleting logs:', error);
            this.showError('Failed to delete logs: ' + error.message);
        }
    }

    async optimizeDatabase() {
        this.showConfirmation(
            'Optimize Database',
            'This will rebuild indexes for all collections. The operation may take some time.',
            () => this.executeOptimize()
        );
    }

    async executeOptimize() {
        try {
            this.showLoading('Optimizing database...');
            
            const response = await fetch('/api/mongodb/optimize', { method: 'POST' });
            const result = await response.json();
            
            if (response.ok) {
                this.showSuccess(result.message);
                await this.loadInitialData();
            } else {
                this.showError('Failed to optimize: ' + result.error);
            }
        } catch (error) {
            console.error('Error optimizing database:', error);
            this.showError('Failed to optimize database: ' + error.message);
        }
    }

    confirmClearAll() {
        this.showConfirmation(
            'Clear All Data',
            'WARNING: This will permanently delete ALL MongoDB collections and data. This action cannot be undone!',
            () => this.executeClearAll()
        );
    }

    async executeClearAll() {
        try {
            this.showLoading('Clearing all data...');
            
            const response = await fetch('/api/mongodb/clear-all', { method: 'DELETE' });
            const result = await response.json();
            
            if (response.ok) {
                this.showSuccess(`${result.message} (${result.count} collections deleted)`);
                await this.loadInitialData();
                document.getElementById('collectionDropdown').value = '';
                document.getElementById('collectionDetails').innerHTML = '';
            } else {
                this.showError('Failed to clear data: ' + result.error);
            }
        } catch (error) {
            console.error('Error clearing data:', error);
            this.showError('Failed to clear data: ' + error.message);
        }
    }

    confirmResetIndexes() {
        this.showConfirmation(
            'Reset All Indexes',
            'This will drop and recreate all indexes. This may improve performance but will take time.',
            () => this.executeOptimize() // Same as optimize for now
        );
    }

    exportStats() {
        const stats = {
            timestamp: new Date().toISOString(),
            status: this.status,
            collections: this.collections
        };
        
        const blob = new Blob([JSON.stringify(stats, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `mongodb-stats-${new Date().toISOString().split('T')[0]}.json`;
        a.click();
        URL.revokeObjectURL(url);
        
        this.showSuccess('Statistics exported successfully');
    }

    // Utility methods
    formatBytes(bytes) {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }

    showLoading(message) {
        console.log('Loading:', message);
        
        // Show loading state in relevant sections
        const sections = ['dbStats', 'storageStats', 'collectionsContent', 'statusContent'];
        sections.forEach(sectionId => {
            const element = document.getElementById(sectionId);
            if (element) {
                element.innerHTML = `<div class="loading">${message}</div>`;
            }
        });
    }

    hideLoading() {
        // Loading states will be replaced by actual content in load methods
        console.log('Loading complete');
    }

    showSuccess(message) {
        this.showAlert(message, 'success');
    }

    showError(message) {
        this.showAlert(message, 'error');
    }

    showAlert(message, type) {
        // Create a temporary alert div
        const alert = document.createElement('div');
        alert.className = type;
        alert.innerHTML = message;
        alert.style.position = 'fixed';
        alert.style.top = '20px';
        alert.style.right = '20px';
        alert.style.zIndex = '9999';
        alert.style.maxWidth = '400px';
        alert.style.borderRadius = '6px';
        alert.style.padding = '1rem';
        
        document.body.appendChild(alert);
        
        setTimeout(() => {
            alert.remove();
        }, 5000);
    }

    showConfirmation(title, message, onConfirm) {
        document.getElementById('confirmTitle').textContent = title;
        document.getElementById('confirmMessage').textContent = message;
        
        this.confirmCallback = onConfirm;
        
        document.getElementById('confirmationDialog').style.display = 'flex';
    }

    hideConfirmation() {
        document.getElementById('confirmationDialog').style.display = 'none';
        this.confirmCallback = null;
    }

    executeConfirmedAction() {
        if (this.confirmCallback) {
            this.confirmCallback();
            this.hideConfirmation();
        }
    }
}

// Initialize the admin interface
const mongoAdmin = new MongoDBAdmin();
