function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

class FieldsRenderer {
    static renderLogFields(log) {
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
                    <div class="log-field-key">${escapeHtml(key)}:</div>
                    <div class="log-field-value">${escapeHtml(String(value))}</div>
                </div>
            `;
        });
        html += '</div>';

        return html;
    }

    static renderExtensions(extensions, logIndex = 0, isNested = false) {
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
                            <button class="mermaid-toggle-btn" onclick="event.stopPropagation(); FieldsRenderer.toggleMermaidRaw('${mermaidId}')" title="Toggle raw text">
                                <span id="toggle-icon-${mermaidId}">👁️</span>
                            </button>
                        </div>
                        <div class="mermaid-diagram" id="diagram-${mermaidId}">
                            <pre class="mermaid">${escapeHtml(diagram)}</pre>
                        </div>
                        <div class="mermaid-raw" id="raw-${mermaidId}" style="display: none;">
                            <pre class="mermaid-raw-text">${escapeHtml(diagram)}</pre>
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
                        <div class="extension-title json-toggle" onclick="FieldsRenderer.toggleJsonSection('${jsonId}')">
                            <span class="json-toggle-icon" id="icon-${jsonId}">▶</span>
                            📄 JSON Data: ${key}
                        </div>
                        <div class="json-content json-collapsible" id="${jsonId}" style="display: none;">
                `;
                
                if (jsonData && typeof jsonData === 'object' && jsonData.raw) {
                    // Render JSON fields 
                    html += '<div class="log-fields">';
                    html += this.renderJsonFields(jsonData.raw, '', jsonId);
                    
                    // Note: Nested Mermaid diagrams are now rendered inline within the JSON structure
                    // Only render non-Mermaid extensions here if needed
                    if (jsonData.extensions && Object.keys(jsonData.extensions).length > 0) {
                        // Filter out mermaid extensions as they're handled inline
                        const nonMermaidExtensions = { ...jsonData.extensions };
                        delete nonMermaidExtensions.mermaid;
                        
                        if (Object.keys(nonMermaidExtensions).length > 0) {
                            html += this.renderExtensions(nonMermaidExtensions, 0, true);
                        }
                    }
                    html += '</div>';
                } else {
                    // Handle simple JSON value
                    html += `<pre class="json-display">${escapeHtml(JSON.stringify(jsonData, null, 2))}</pre>`;
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
        //                 <div class="log-field-key">${escapeHtml(key)}:</div>
        //                 <div class="log-field-value">${escapeHtml(String(value))}</div>
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
    static flattenExtensions(extensions, result = {}) {
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

    static renderJsonFields(obj, keyPrefix = '', jsonId) {
        let html = '';
        
        if (typeof obj !== 'object' || obj === null) {
            return `<div class="log-field">
                        <div class="log-field-key">${escapeHtml(keyPrefix)}:</div>
                        <div class="log-field-value">${escapeHtml(String(obj))}</div>
                    </div>`;
        }
        
        for (const [key, value] of Object.entries(obj)) {
            const fullKey = keyPrefix ? `${keyPrefix}.${key}` : key;
            
            if (typeof value === 'object' && value !== null) {
                // Recursively render nested objects
                const recursiveJsonId = `${jsonId}-${fullKey}`;
                html += `
                    <div class="json-toggle" onclick="FieldsRenderer.toggleJsonSection('${recursiveJsonId}')">
                        <span class="json-toggle-icon" id="icon-${recursiveJsonId}">▶</span>
                        ${key}
                    </div>
                    <div class="json-content json-collapsible" id="${recursiveJsonId}" style="display: none;">
                `;
                html += this.renderJsonFields(value, fullKey, jsonId);
                html += '</div>';
            } else {
                // Handle extension fields inline
                if (fullKey.endsWith('.mermaid')) {
                    // Render Mermaid diagram inline
                    const mermaidId = `mermaid-${jsonId}-${fullKey.replace(/\./g, '-')}-${Math.random().toString(36).substr(2, 9)}`;
                    html += `
                        <div class="log-field">
                            <div class="log-field-key">${escapeHtml(key)}:</div>
                            <div class="log-field-value">
                                <div class="extension-content">
                                    <div class="extension-title">
                                        📊 Mermaid Diagram
                                        <button class="mermaid-toggle-btn" onclick="event.stopPropagation(); FieldsRenderer.toggleMermaidRaw('${mermaidId}')" title="Toggle raw text">
                                            <span id="toggle-icon-${mermaidId}">👁️</span>
                                        </button>
                                    </div>
                                    <div class="mermaid-diagram" id="diagram-${mermaidId}">
                                        <pre class="mermaid">${escapeHtml(value)}</pre>
                                    </div>
                                    <div class="mermaid-raw" id="raw-${mermaidId}" style="display: none;">
                                        <pre class="mermaid-raw-text">${escapeHtml(value)}</pre>
                                    </div>
                                </div>
                            </div>
                        </div>
                    `;
                } else if (fullKey.endsWith('.json') || fullKey.includes('.json.')) {
                    // Skip JSON extension fields as they're handled by the main extension system
                } else {
                    console.log("regular field", key, fullKey, value);
                    // Render as regular field
                    const displayValue = Array.isArray(value) ? JSON.stringify(value) : String(value);
                    html += `
                        <div class="log-field">
                            <div class="log-field-key">${escapeHtml(key)}:</div>
                            <div class="log-field-value">${escapeHtml(displayValue)}</div>
                        </div>
                    `;
                }
            }
        }
        
        return html;
    }

    static renderPanicInfo(meta) {
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
                    html += `<div class="backtrace">${escapeHtml(backtrace)}</div>`;
                } else {
                    html += '<p>No meaningful stack frames found</p>';
                }
            }
        }

        html += '</div>';
        return html;
    }

    static async triggerMermaidRendering(container = null) {
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

    static isDiagramVisible(diagram) {
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

    static toggleJsonSection(jsonId) {
        const section = document.getElementById(jsonId);
        const icon = document.getElementById(`icon-${jsonId}`);
        
        if (section && icon) {
            if (section.style.display === 'none') {
                section.style.display = 'block';
                icon.textContent = '▼';
                
                // Add click event listeners to Mermaid diagrams in this section
                section.querySelectorAll('.mermaid-diagram').forEach((diagram) => {
                    diagram.addEventListener('click', (e) => {
                        e.stopPropagation();
                        // Get the Mermaid text from the corresponding raw text element
                        const mermaidId = diagram.id;
                        const rawElement = document.getElementById(`raw-${mermaidId}`);
                        if (rawElement) {
                            const mermaidRawText = rawElement.querySelector('.mermaid-raw-text');
                            if (mermaidRawText) {
                                FieldsRenderer.showMermaidModal(mermaidId, mermaidRawText.textContent);
                            }
                        }
                    });
                });
                
                // Trigger Mermaid rendering only for this specific JSON section
                setTimeout(() => this.triggerMermaidRendering(section), 100);
            } else {
                section.style.display = 'none';
                icon.textContent = '▶';
            }
        }
    }

    static toggleMermaidRaw(mermaidId) {
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

    static showMermaidModal(mermaidId, diagramText) {
        const modal = document.getElementById('mermaidModal');
        const modalDiagram = document.getElementById('mermaidModalDiagram');
        const modalRaw = document.getElementById('mermaidModalRaw');
        const modalRawText = document.getElementById('mermaidModalRawText');
        const toggleBtn = document.getElementById('toggleMermaidModal');
        
        // Store the original diagram text
        this.currentModalDiagramText = diagramText;
        
        // Set up the modal content
        modalDiagram.innerHTML = `<pre class="mermaid">${diagramText}</pre>`;
        modalRawText.textContent = diagramText;
        
        // Reset modal state
        modalDiagram.style.display = 'block';
        modalRaw.style.display = 'none';
        toggleBtn.textContent = 'Show Raw Text';
        
        // Show the modal
        modal.classList.add('active');
        
        // Trigger Mermaid rendering for the modal
        setTimeout(() => {
            FieldsRenderer.triggerMermaidRendering(modalDiagram);
        }, 100);
    }

    static toggleMermaidModal() {
        const modalDiagram = document.getElementById('mermaidModalDiagram');
        const modalRaw = document.getElementById('mermaidModalRaw');
        const toggleBtn = document.getElementById('toggleMermaidModal');
        
        if (modalRaw.style.display === 'none') {
            // Show raw text, hide diagram
            modalDiagram.style.display = 'none';
            modalRaw.style.display = 'block';
            toggleBtn.textContent = 'Show Diagram';
        } else {
            // Show diagram, hide raw text
            modalDiagram.style.display = 'block';
            modalRaw.style.display = 'none';
            toggleBtn.textContent = 'Show Raw Text';
            // Trigger Mermaid rendering when switching back to diagram
            setTimeout(() => {
                FieldsRenderer.triggerMermaidRendering(modalDiagram);
            }, 100);
        }
    }
}
