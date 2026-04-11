#!/usr/bin/env node
/**
 * Generate research report from JSON results.
 * Run: node review-prompts-research/generate_report.py
 */
const fs = require('fs');
const path = require('path');

const RESULTS_DIR = path.join(__dirname, 'results');
const FIELDS_PATH = path.join(__dirname, 'fields.yaml');
const OUTPUT_PATH = path.join(__dirname, 'report.md');

// Parse fields.yaml manually (simple structure)
function parseFieldsYaml(content) {
  const fields = [];
  const blocks = content.split(/^\s+-\s+name:/m);
  for (let i = 1; i < blocks.length; i++) {
    const nameMatch = blocks[i].match(/^(.+)/);
    const descMatch = blocks[i].match(/description:\s*>?\s*\n?\s*(.+(?:\n\s{6,}.+)*)/);
    if (nameMatch) {
      fields.push({
        name: nameMatch[1].trim(),
        description: descMatch ? descMatch[1].replace(/\n\s+/g, ' ').trim() : ''
      });
    }
  }
  return fields;
}

// Format a value for markdown
function formatValue(val, indent = '') {
  if (val === null || val === undefined || val === '') return '_N/A_';
  if (typeof val === 'string') {
    if (val.includes('[uncertain]')) return null; // skip
    return val.length > 150 ? `\n${indent}> ${val}` : val;
  }
  if (Array.isArray(val)) {
    const items = val.filter(item => {
      if (typeof item === 'string' && item.includes('[uncertain]')) return false;
      return true;
    });
    if (items.length === 0) return '_N/A_';
    return '\n' + items.map(item => {
      if (typeof item === 'string') {
        return `${indent}- ${item}`;
      }
      if (typeof item === 'object' && item !== null) {
        const parts = Object.entries(item)
          .filter(([k]) => k !== 'uncertain')
          .map(([k, v]) => {
            if (typeof v === 'string' && v.length > 100) return `**${k}:** ${v}`;
            return `**${k}:** ${v}`;
          });
        return `${indent}- ${parts.join(' | ')}`;
      }
      return `${indent}- ${String(item)}`;
    }).join('\n');
  }
  if (typeof val === 'object') {
    return '\n' + Object.entries(val)
      .filter(([k]) => !['uncertain', '_source_file'].includes(k))
      .map(([k, v]) => `${indent}- **${k}:** ${typeof v === 'string' ? v : JSON.stringify(v)}`)
      .join('\n');
  }
  return String(val);
}

// Slugify for anchors
function slugify(str) {
  return str.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '');
}

// Main
const fieldsContent = fs.readFileSync(FIELDS_PATH, 'utf8');
const fields = parseFieldsYaml(fieldsContent);
const fieldNames = fields.map(f => f.name);

const jsonFiles = fs.readdirSync(RESULTS_DIR).filter(f => f.endsWith('.json')).sort();
const items = jsonFiles.map(f => {
  const data = JSON.parse(fs.readFileSync(path.join(RESULTS_DIR, f), 'utf8'));
  const name = f.replace('.json', '').replace(/_/g, ' ');
  return { name, data, file: f };
});

// Build report
let md = '# Research Report: Review Prompts Improvement\n\n';
md += '> Research on Claude Code review prompt best practices and prompt engineering\n';
md += '> to split orbflow-review-prompts.md into frontend and Rust backend files.\n\n';

// TOC
md += '## Table of Contents\n\n';
items.forEach((item, i) => {
  const principleCount = Array.isArray(item.data.key_principles) ? item.data.key_principles.length : 0;
  const antiPatternCount = Array.isArray(item.data.anti_patterns) ? item.data.anti_patterns.length : 0;
  md += `${i + 1}. [${item.name}](#${slugify(item.name)}) — ${principleCount} principles, ${antiPatternCount} anti-patterns\n`;
});
md += '\n---\n\n';

// Detailed content
items.forEach((item, i) => {
  md += `## ${i + 1}. ${item.name}\n\n`;

  // Known fields
  for (const field of fields) {
    // Look up value: top-level first, then nested
    let val = item.data[field.name];
    if (val === undefined) {
      // Search nested dicts
      for (const [k, v] of Object.entries(item.data)) {
        if (typeof v === 'object' && v !== null && !Array.isArray(v) && field.name in v) {
          val = v[field.name];
          break;
        }
      }
    }

    // Check uncertain array
    const uncertainArr = item.data.uncertain || [];
    if (uncertainArr.includes(field.name)) continue;
    if (val === undefined || val === null) continue;

    const formatted = formatValue(val, '  ');
    if (formatted === null) continue; // was [uncertain]

    md += `### ${field.name.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())}\n\n`;
    md += `${formatted}\n\n`;
  }

  // Extra fields not in fields.yaml
  const knownKeys = new Set([...fieldNames, 'uncertain', '_source_file']);
  const extraKeys = Object.keys(item.data).filter(k => !knownKeys.has(k));
  if (extraKeys.length > 0) {
    md += `### Other Info\n\n`;
    for (const k of extraKeys) {
      const formatted = formatValue(item.data[k], '  ');
      if (formatted === null) continue;
      md += `**${k}:** ${formatted}\n\n`;
    }
  }

  // Uncertain fields
  if (item.data.uncertain && item.data.uncertain.length > 0) {
    md += `### Uncertain Fields\n\n`;
    item.data.uncertain.forEach(f => { md += `- ${f}\n`; });
    md += '\n';
  }

  md += '---\n\n';
});

fs.writeFileSync(OUTPUT_PATH, md, 'utf8');
console.log(`Report generated: ${OUTPUT_PATH}`);
console.log(`Items: ${items.length}`);
console.log(`Fields per item: ${fields.length}`);
