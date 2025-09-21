class FieldParser {
    constructor() {
        this.initializeApp();
        this.bindEvents();
    }

    initializeApp() {
        console.log('Field Parser initialized');
    }

    bindEvents() {
        // Form submission
        document.getElementById('fieldParserForm').addEventListener('submit', (e) => {
            e.preventDefault();
            this.parseFields();
        });

        // Clear button
        document.getElementById('clearBtn').addEventListener('click', () => {
            this.clearForm();
        });

        // Auto-parse on input change (optional)
        document.getElementById('fieldsInput').addEventListener('input', (e) => {
            // Debounce the parsing to avoid too many calls
            clearTimeout(this.parseTimeout);
            this.parseTimeout = setTimeout(() => {
                if (e.target.value.trim()) {
                    this.parseFields();
                }
            }, 500);
        });

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

    parseFields() {
        const input = document.getElementById('fieldsInput').value.trim();
        const outputContent = document.getElementById('outputContent');

        if (!input) {
            this.showError('Please enter some fields JSON');
            return;
        }

        try {
            // Parse the JSON input
            const fields = JSON.parse(input);

            // Create a mock extensions object as if it's a JSON extension
            const mockExtensions = {
                json: {
                    'field_parser_input': {
                        raw: fields,
                        extensions: {}
                    }
                }
            };

            // Use the renderExtensions method from FieldsRenderer
            const renderedExtensions = FieldsRenderer.renderExtensions(mockExtensions, 0);

            if (renderedExtensions) {
                outputContent.innerHTML = renderedExtensions;
                // Trigger Mermaid rendering for any diagrams in the output
                setTimeout(() => FieldsRenderer.triggerMermaidRendering(outputContent), 100);
            } else {
                outputContent.innerHTML = '<div class="output-placeholder">No fields to display (all fields were filtered out)</div>';
            }
            outputContent.querySelectorAll('.mermaid-diagram').forEach((diagram) => {
                diagram.addEventListener('click', (e) => {
                    e.stopPropagation();
                    // Get the Mermaid text from the corresponding raw text element
                    const mermaidId = diagram.id.replace('diagram-', '');
                    const rawElement = document.getElementById(`raw-${mermaidId}`);
                    if (rawElement) {
                        const mermaidRawText = rawElement.querySelector('.mermaid-raw-text');
                        if (mermaidRawText) {
                            FieldsRenderer.showMermaidModal(mermaidId, mermaidRawText.textContent);
                        }
                    }
                });
            });

        } catch (error) {
            this.showError(`Invalid JSON: ${error.message}`);
        }
    }

    clearForm() {
        document.getElementById('fieldsInput').value = '';
        document.getElementById('outputContent').innerHTML = '<div class="output-placeholder">Enter fields JSON and click "Parse Fields" to see the rendered output</div>';
    }

    showError(message) {
        const outputContent = document.getElementById('outputContent');
        outputContent.innerHTML = `<div class="error-message">${escapeHtml(message)}</div>`;
    }
}

// Initialize the field parser when the page loads
document.addEventListener('DOMContentLoaded', () => {
    new FieldParser();
});
